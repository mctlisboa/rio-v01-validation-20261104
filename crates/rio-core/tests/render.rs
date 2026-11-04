use rio_core::*;
fn fixture(name: &str) -> Report {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
#[test]
fn terminal_and_markdown_snapshots() {
    let policy = FailOn::default().compile().unwrap();
    for name in ["complete", "incomplete", "partial", "no_changes", "error"] {
        let mut report = fixture(name);
        if name == "incomplete" {
            report.findings.clear();
            report.risks.clear();
        }
        for width in [80, 120] {
            let options = RenderOptions {
                width,
                show_pre_existing: true,
                policy: Some(&policy),
                ..Default::default()
            };
            let output = render_terminal(&report, &options);
            assert!(output.lines().all(|line| line.chars().count() <= width));
            insta::assert_snapshot!(format!("{name}_{width}"), output);
        }
        insta::assert_snapshot!(
            format!("{name}_markdown"),
            render_markdown(&report, &RenderOptions::default())
        );
    }
}
#[test]
fn json_validates_all_statuses_and_rejects_invalid_reports_before_output() {
    for name in ["complete", "incomplete", "partial", "no_changes", "error"] {
        let report = fixture(name);
        let output = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert!(parsed["metrics"]["wall_time_ms"].is_number());
        for check in parsed["checks"].as_array().unwrap() {
            assert!(check["wall_time_ms"].is_number());
        }
    }
    assert!(render_json(&Report::empty(ReportStatus::Error)).is_err());
}
#[test]
fn every_renderer_redacts_credentials_and_uses_stable_headings() {
    let mut report = fixture("complete");
    report
        .footer
        .notes
        .push("token = ghp_abcdefghijklmnopqrstuvwxyz1234567890".into());
    for output in [
        render_terminal(&report, &RenderOptions::default()),
        render_markdown(&report, &RenderOptions::default()),
        String::from_utf8(render_json(&report).unwrap()).unwrap(),
    ] {
        assert!(!output.contains("abcdefghijklmnopqrstuvwxyz"));
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains('\x1b'));
    }
    let first = render_markdown(&report, &RenderOptions::default());
    let second = render_markdown(&fixture("partial"), &RenderOptions::default());
    for heading in ["## Deterministic results", "## Findings", "## Footer"] {
        assert!(first.contains(heading));
        assert!(second.contains(heading));
    }
}
#[test]
fn progress_reports_completed_outcomes() {
    assert!(
        render_progress(&ReviewEvent::CheckFinished {
            kind: CheckKind::Coverage,
            root: "pkg".into(),
            findings: 0,
            skipped: false,
            outcome: CheckOutcome::Cancelled {
                reason: CancellationReason::Timeout,
                details: CheckDetails::default()
            }
        })
        .unwrap()
        .contains("cancelled (timeout)")
    );
}
