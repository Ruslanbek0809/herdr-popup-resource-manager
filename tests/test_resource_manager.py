"""Unit tests: python3 -m unittest discover -s tests -v (from the repo root).

Fixtures are real output captured on macOS with herdr 0.9.3: `session.snapshot`,
`pane.process_info` per pane, and `ps -A -o pid=,ppid=,time=,rss=,comm=`.
"""

import curses
import importlib.machinery
import importlib.util
import json
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURES = os.path.join(ROOT, "tests", "fixtures")

_loader = importlib.machinery.SourceFileLoader("rm", os.path.join(ROOT, "herdr-resource-manager"))
_spec = importlib.util.spec_from_loader("rm", _loader)
rm = importlib.util.module_from_spec(_spec)
sys.modules["rm"] = rm  # dataclasses look the module up by name
_loader.exec_module(rm)


def fixture(name):
    with open(os.path.join(FIXTURES, name), encoding="utf-8") as f:
        return json.load(f) if name.endswith(".json") else f.read()


SNAPSHOT = fixture("snapshot.json")
INFOS = fixture("process_info.json")
PROCS = rm.parse_ps(fixture("ps.txt"))
NO_PID = 999_999  # a pid that isn't in the fixture, so no pane is excluded


def find(nodes, node_id):
    for n in nodes:
        if n.id == node_id:
            return n
        hit = find(n.children, node_id)
        if hit:
            return hit
    return None


class ParseTest(unittest.TestCase):
    def test_cpu_time_formats(self):
        self.assertAlmostEqual(rm.parse_cpu_time("0:02.93"), 2.93)
        self.assertAlmostEqual(rm.parse_cpu_time("3133:45.79"), 3133 * 60 + 45.79)
        self.assertAlmostEqual(rm.parse_cpu_time("02:03:04"), 2 * 3600 + 3 * 60 + 4)
        self.assertAlmostEqual(rm.parse_cpu_time("1-02:03:04"), 86400 + 2 * 3600 + 3 * 60 + 4)

    def test_ps_fixture(self):
        self.assertEqual(len(PROCS), 62)
        claude = PROCS[12032]
        self.assertEqual((claude.ppid, claude.rss, claude.comm), (11959, 255424 * 1024, "claude"))
        self.assertAlmostEqual(claude.cpu, 32.39)

    def test_comm_with_spaces_kept_whole(self):
        procs = rm.parse_ps("  10     1   0:01.00   2048 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome\n")
        self.assertEqual(procs[10].comm, "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")

    def test_app_name_groups_helpers(self):
        self.assertEqual(rm.app_name("/Applications/Google Chrome.app/Contents/Frameworks/x/Helper (Renderer)"),
                         "Google Chrome")
        self.assertEqual(rm.app_name("/usr/sbin/systemstats"), "systemstats")


class ProcReaderTest(unittest.TestCase):
    def test_names_fetched_once_per_new_pid(self):
        tables = iter(["  1  0  0:01.00  100\n  2  1  0:00.50  200\n",
                       "  1  0  0:01.10  100\n  3  1  0:00.00  300\n"])
        name_calls = []

        def ps(*args):
            if args[0] == "-A":
                return next(tables)
            name_calls.append(args[-1])
            known = {"1": "/sbin/launchd", "2": "/usr/bin/yes", "3": "/Applications/A B.app/Contents/MacOS/A B"}
            return "".join(f"{pid} {known[pid]}\n" for pid in args[-1].split(","))

        reader = rm.ProcReader(ps)
        first = reader()
        self.assertEqual((first[2].comm, first[2].rss), ("/usr/bin/yes", 200 * 1024))
        second = reader()
        self.assertEqual(name_calls, ["1,2", "3"])
        self.assertEqual(second[3].comm, "/Applications/A B.app/Contents/MacOS/A B")
        self.assertNotIn(2, reader.names)  # exited pids are forgotten


class UsageTest(unittest.TestCase):
    def procs(self, **cpu):
        return {int(pid[1:]): rm.Proc(int(pid[1:]), 1, c, 1024, "x") for pid, c in cpu.items()}

    def test_first_sample_has_no_cpu(self):
        self.assertIsNone(rm.usage_of([2], self.procs(p2=5.0), None, None).cpu)

    def test_per_pid_delta(self):
        prev = {2: 1.0, 3: 50.0}  # pid 3 used 50 s and has exited since
        now = self.procs(p2=1.5)
        self.assertAlmostEqual(rm.usage_of([2, 3], now, prev, 1.0).cpu, 50.0)

    def test_new_pid_counts_from_zero(self):
        self.assertAlmostEqual(rm.usage_of([4], self.procs(p4=0.2), {}, 1.0).cpu, 20.0)

    def test_reused_pid_never_negative(self):
        self.assertEqual(rm.usage_of([2], self.procs(p2=0.1), {2: 9.0}, 1.0).cpu, 0.0)

    def test_sum_keeps_unmeasured_as_none(self):
        self.assertIsNone((rm.Usage(None, 1) + rm.Usage(None, 2)).cpu)
        self.assertEqual((rm.Usage(None, 1) + rm.Usage(3.0, 2)), rm.Usage(3.0, 3))


class TreeTest(unittest.TestCase):
    def build(self, own_pid=NO_PID, prev=None, dt=None):
        return rm.build_tree(SNAPSHOT, INFOS, PROCS, prev, dt, own_pid)

    def test_shape_follows_snapshot_order(self):
        nodes, _ = self.build()
        self.assertEqual([n.id for n in nodes], ["w8", "w9", "outside"])
        self.assertEqual([t.id for t in nodes[1].children], ["w9:t1", "w9:t5", "w9:t7"])

    def test_pane_sums_its_process_tree(self):
        nodes, _ = self.build()
        # shell 11959 → claude 12032 → dart 13287 → dart 13348
        self.assertEqual(find(nodes, "w8:p1").usage.rss, (16 + 255424 + 3952 + 40080) * 1024)

    def test_parents_sum_children_and_total_excludes_outside(self):
        nodes, total = self.build()
        ws = find(nodes, "w9")
        self.assertEqual(ws.usage.rss, sum(t.usage.rss for t in ws.children))
        self.assertEqual(total.rss, find(nodes, "w8").usage.rss + ws.usage.rss)

    def test_pane_names(self):
        nodes, _ = self.build()
        p1 = find(nodes, "w8:p1")
        self.assertEqual((p1.label, p1.detail, p1.status), ("claude", "Review commits and clean up", "idle"))
        shell = find(nodes, "w9:pD")
        self.assertEqual((shell.label, shell.detail, shell.status), ("shell", "zsh", ""))

    def test_own_pane_excluded(self):
        nodes, _ = self.build(own_pid=46311)  # a zsh under claude 34372 in pane w8:pN
        self.assertIsNone(find(nodes, "w8:pN"))
        self.assertIsNone(find(nodes, "w8:tC"))
        self.assertIsNotNone(find(nodes, "w8:p1"))

    def test_outside_has_every_unclaimed_process(self):
        nodes, total = self.build()
        outside = find(nodes, "outside")
        self.assertEqual(total.rss + outside.usage.rss, sum(p.rss for p in PROCS.values()))
        self.assertEqual(len(outside.children), rm.OUTSIDE_TOP)

    def test_cpu_after_second_sample(self):
        prev = {p.pid: p.cpu for p in PROCS.values()}
        prev[12032] -= 0.5  # claude in w8:p1 used half a core over 1 s
        nodes, _ = self.build(prev=prev, dt=1.0)
        self.assertAlmostEqual(find(nodes, "w8:p1").usage.cpu, 50.0)
        self.assertAlmostEqual(find(nodes, "w9:p1").usage.cpu, 0.0)

    def test_pane_without_process_info(self):
        infos = dict(INFOS, **{"w9:p1": None})
        nodes, _ = rm.build_tree(SNAPSHOT, infos, PROCS, None, None, NO_PID)
        pane = find(nodes, "w9:p1")
        self.assertEqual((pane.label, pane.usage.rss), ("claude", 0))

    def test_linked_worktrees_follow_their_primary(self):
        def ws(wid, key=None, linked=False):
            out = {"workspace_id": wid, "label": wid}
            if key:
                out["worktree"] = {"repo_key": key, "is_linked_worktree": linked}
            return out

        order = rm.order_workspaces([ws("a", "r1", True), ws("b"), ws("c", "r1"), ws("d", "r2", True)])
        self.assertEqual([(w["workspace_id"], linked) for w, linked in order],
                         [("b", False), ("c", False), ("a", True), ("d", True)])

    def test_flatten_respects_folds(self):
        nodes, _ = self.build()
        all_rows = rm.flatten(nodes, set())
        folded = rm.flatten(nodes, {"w9"})
        hidden = len(find(nodes, "w9").children) + sum(len(t.children) for t in find(nodes, "w9").children)
        self.assertEqual(len(all_rows) - len(folded), hidden)
        self.assertEqual([r.depth for r in all_rows[:3]], [0, 1, 2])


class FormatTest(unittest.TestCase):
    def test_bytes(self):
        self.assertEqual(rm.format_bytes(784 * 1024), "784 KB")
        self.assertEqual(rm.format_bytes(int(180.3 * 1024 ** 2)), "180.3 MB")
        self.assertEqual(rm.format_bytes(int(1.58 * 1024 ** 3)), "1.58 GB")

    def test_cpu(self):
        self.assertEqual(rm.format_cpu(None), "…")
        self.assertEqual(rm.format_cpu(67.24), "67.2%")

    def test_sparkline(self):
        self.assertEqual(rm.sparkline([5, 5, 5], 10), "▁▁▁")
        self.assertEqual(rm.sparkline([0, 7, 14], 10), "▁▅█")
        self.assertEqual(rm.sparkline(range(0, 100, 10), 4), "▁▃▆█")  # last 4: 60..90
        self.assertEqual(rm.sparkline([], 4), "")
        self.assertEqual(rm.sparkline([1000, 1001, 1000], 4), "▁▁▁")  # noise stays flat
        self.assertEqual(rm.sparkline([0, 0], 4), "▁▁")

    def test_truncate_is_width_aware(self):
        self.assertEqual(rm.truncate("fh-10354-bound", 8), "fh-1035…")
        self.assertEqual(rm.truncate("short", 8), "short")
        self.assertEqual(rm.truncate("日本語テキスト", 6), "日本…")
        self.assertLessEqual(rm.text_width(rm.truncate("日本語テキスト", 6)), 6)
        self.assertEqual(rm.truncate("Ýüzlenme ›", 20), "Ýüzlenme ›")


class SamplerTest(unittest.TestCase):
    def test_two_samples_give_cpu(self):
        clock = iter([10.0, 11.0])
        busy = {"n": 0}

        def ps():
            procs = dict(PROCS)
            p = procs[12174]  # claude in w9:p1
            procs[12174] = p._replace(cpu=p.cpu + busy["n"])
            busy["n"] += 2  # two cores busy for one second
            return procs

        def request(method, params=None):
            if method == "session.snapshot":
                return {"snapshot": SNAPSHOT}
            if params["pane_id"] == "w9:p5":
                raise rm.HerdrError("pane not found")
            return {"process_info": INFOS[params["pane_id"]]}

        sampler = rm.Sampler(request, ps, lambda: next(clock), own_pid=NO_PID)
        first = sampler.sample()
        self.assertIsNone(find(first.nodes, "w9:p1").usage.cpu)
        second = sampler.sample()
        self.assertAlmostEqual(find(second.nodes, "w9:p1").usage.cpu, 200.0)
        self.assertEqual(find(second.nodes, "w9:p5").usage.rss, 0)


class KeysTest(unittest.TestCase):
    """Key handling without a terminal: the App is built with no screen."""

    def setUp(self):
        self.calls = []

        def request(method, params=None):
            self.calls.append((method, params))
            return {}

        sampler = rm.Sampler(lambda m, p=None: {"snapshot": SNAPSHOT} if m == "session.snapshot"
                             else {"process_info": INFOS[p["pane_id"]]},
                             lambda: PROCS, own_pid=NO_PID)
        self.app = rm.App(None, sampler, request)
        self.app.refresh_data()

    def select(self, node_id):
        self.app.selected = node_id

    def test_enter_on_pane_focuses_it_then_quits(self):
        self.select("w9:p1")
        self.assertFalse(self.app.handle_key("\n"))
        self.assertEqual(self.calls, [("pane.focus", {"pane_id": "w9:p1"})])

    def test_enter_on_tab_focuses_tab(self):
        self.select("w9:t5")
        self.assertFalse(self.app.handle_key("\n"))
        self.assertEqual(self.calls, [("tab.focus", {"tab_id": "w9:t5"})])

    def test_enter_on_workspace_focuses_workspace_only(self):
        self.select("w8")
        self.assertFalse(self.app.handle_key("\n"))
        self.assertEqual(self.calls, [("workspace.focus", {"workspace_id": "w8"})])

    def test_enter_on_outside_does_nothing(self):
        self.select("outside")
        self.assertTrue(self.app.handle_key("\n"))
        self.assertEqual(self.calls, [])

    def test_failed_focus_keeps_popup_open(self):
        def failing(method, params=None):
            raise rm.HerdrError("pane.focus: nope")

        self.app.request = failing
        self.select("w9:p1")
        self.assertTrue(self.app.handle_key("\n"))
        self.assertIn("nope", self.app.message)

    def test_x_asks_then_closes_only_on_y(self):
        self.select("w9:pD")
        self.app.handle_key("x")
        self.assertEqual(self.app.confirm.pane_id, "w9:pD")
        self.app.handle_key("n")
        self.assertEqual(self.calls, [])
        self.app.handle_key("x")
        self.app.handle_key("y")
        self.assertEqual(self.calls, [("pane.close", {"pane_id": "w9:pD"})])

    def test_x_on_workspace_is_a_hint(self):
        self.select("w9")
        self.app.handle_key("x")
        self.assertIsNone(self.app.confirm)
        self.assertTrue(self.app.message)

    def test_move_and_fold(self):
        self.select("w8")
        self.app.handle_key(curses.KEY_DOWN)
        self.assertEqual(self.app.selected, "w8:t1")
        self.app.handle_key(curses.KEY_UP)
        self.app.handle_key(" ")
        self.assertIn("w8", self.app.folded)
        self.app.handle_key(curses.KEY_DOWN)
        self.assertEqual(self.app.selected, "w9")

    def test_read_key_decodes_both_arrow_forms(self):
        class Scr:
            def __init__(self, keys):
                self.keys = list(keys)

            def get_wch(self):
                if not self.keys:
                    raise curses.error("no input")
                return self.keys.pop(0)

            def nodelay(self, flag):
                pass

            def timeout(self, ms):
                pass

        for keys, want in [("\x1b[B", curses.KEY_DOWN), ("\x1bOA", curses.KEY_UP),
                           ("\x1b", "\x1b"), ("\x1b[5", None), ("j", "j")]:
            self.app.scr = Scr(keys)
            self.assertEqual(self.app.read_key(), want, repr(keys))
        self.app.scr = Scr([])
        self.assertIsNone(self.app.read_key())

    def test_selection_stays_in_place_when_its_row_vanishes(self):
        self.select("w9:p1")
        rows = self.app.rows()
        i = self.app.selected_index(rows)
        self.app.selected = "w9:gone"
        self.assertEqual(self.app.selected_index(rows), i)
        self.assertEqual(self.app.selected_index(rows[:3]), 2)

    def test_q_and_esc_quit(self):
        self.assertFalse(self.app.handle_key("q"))
        self.assertFalse(self.app.handle_key("\x1b"))


if __name__ == "__main__":
    unittest.main()
