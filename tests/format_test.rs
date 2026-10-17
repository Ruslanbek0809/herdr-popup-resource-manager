use herdr_popup_resource_manager::format::{
    format_bytes, format_cpu, sparkline, text_width, truncate,
};

#[test]
fn bytes_use_kb_mb_gb_like_the_screenshot() {
    assert_eq!(format_bytes(784 * 1024), "784 KB");
    assert_eq!(format_bytes((180.3 * 1024.0 * 1024.0) as u64), "180.3 MB");
    assert_eq!(
        format_bytes((1.58 * 1024.0 * 1024.0 * 1024.0) as u64),
        "1.58 GB"
    );
}

#[test]
fn cpu_shows_ellipsis_until_measured() {
    assert_eq!(format_cpu(None), "…");
    assert_eq!(format_cpu(Some(67.24)), "67.2%");
}

#[test]
fn sparkline_scales_per_row_and_keeps_noise_flat() {
    assert_eq!(sparkline(&[5, 5, 5], 10), "▁▁▁");
    assert_eq!(sparkline(&[0, 7, 14], 10), "▁▅█");
    assert_eq!(
        sparkline(&[0, 10, 20, 30, 40, 50, 60, 70, 80, 90], 4),
        "▁▃▆█"
    ); // last 4: 60..90
    assert_eq!(sparkline(&[], 4), "");
    assert_eq!(sparkline(&[1000, 1001, 1000], 4), "▁▁▁");
    assert_eq!(sparkline(&[0, 0], 4), "▁▁");
}

#[test]
fn truncate_is_display_width_aware() {
    assert_eq!(truncate("fh-10354-bound", 8), "fh-1035…");
    assert_eq!(truncate("short", 8), "short");
    assert_eq!(truncate("日本語テキスト", 6), "日本…");
    assert!(text_width(&truncate("日本語テキスト", 6)) <= 6);
    assert_eq!(truncate("Ýüzlenme ›", 20), "Ýüzlenme ›");
    assert_eq!(truncate("anything", 0), "");
}
