//! Live, read-only performance diagnostics smoke test.
//!
//! This test does not change startup entries, services, registry values or
//! processes. It verifies that the documented Windows APIs return figures that
//! are internally consistent on a real machine and that every recommendation
//! carries evidence, a risk and a verification method.

use cyron_lib::perf;

#[test]
fn live_snapshot_is_consistent_and_every_claim_has_evidence() {
    let snapshot = perf::capture_snapshot(true);
    println!(
        "cpu={:.1}% cores={} mem={}/{} ({:.1}%) commit={:.1}% disk={:?}/{:?} processes={} startup={}/{} scheduled={} eventLog={} drivers={} systemEvents={} ({})",
        snapshot.cpu.utilization_percent,
        snapshot.cpu.logical_cores,
        snapshot.memory.available_bytes,
        snapshot.memory.total_bytes,
        snapshot.memory.used_percent,
        snapshot.memory.commit_percent,
        snapshot.disk_io.busy_percent,
        snapshot.disk_io.queue_length,
        snapshot.process_count,
        snapshot.startup.enabled_count,
        snapshot.startup.total_count,
        snapshot.startup.scheduled_tasks.len(),
        snapshot.startup.event_log_status,
        snapshot.drivers.items.len(),
        snapshot.system_events.items.len(),
        snapshot.system_events.status
    );

    assert!(snapshot.cpu.logical_cores >= 1);
    assert!(snapshot.cpu.utilization_percent >= 0.0);
    assert!(snapshot.cpu.utilization_percent <= 100.0);
    assert!(
        (snapshot.cpu.user_percent + snapshot.cpu.kernel_percent + snapshot.cpu.idle_percent
            - 100.0)
            .abs()
            < 0.6
    );
    assert!(snapshot.memory.total_bytes > 0);
    assert!(snapshot.memory.available_bytes <= snapshot.memory.total_bytes);
    assert!(!snapshot.volumes.is_empty());
    assert!(snapshot.process_count > 0);
    assert!(!snapshot.limitations.is_empty());
    assert!(!snapshot.system_events.note.is_empty());
    assert!(
        matches!(
            snapshot.system_events.status.as_str(),
            "available" | "denied" | "unavailable"
        ),
        "unexpected System event status: {}",
        snapshot.system_events.status
    );
    for driver in &snapshot.drivers.items {
        assert!(!driver.name.is_empty());
        assert!(matches!(driver.class.as_str(), "Display" | "Storage" | "Network"));
    }
    assert!(
        snapshot.sample_ms >= 500,
        "sample too short: {}",
        snapshot.sample_ms
    );

    let mut seen = std::collections::HashSet::new();
    for process in &snapshot.processes {
        assert!(seen.insert(process.pid), "duplicate PID {}", process.pid);
        assert!((0.0..=100.0).contains(&process.cpu_percent));
    }

    for recommendation in &snapshot.recommendations {
        assert!(
            !recommendation.evidence.is_empty(),
            "{} has no evidence",
            recommendation.id
        );
        assert!(!recommendation.expected_effect.is_empty());
        assert!(!recommendation.risk.is_empty());
        assert!(!recommendation.verification.is_empty());
        assert!(
            recommendation.action.kind == "none"
                || recommendation.action.kind == "view"
                || recommendation.action.kind == "page"
                || (recommendation.action.kind == "startup"
                    && recommendation.action.target_id.is_some()),
            "{} has an unsupported action {}",
            recommendation.id,
            recommendation.action.kind
        );
    }

    for item in &snapshot.startup.items {
        assert!(!item.id.is_empty());
        if item.boot_impact_ms.is_some() {
            assert_eq!(item.evidence_source, "diagnostics-performance");
        }
    }

    let _ = snapshot.startup.boot.as_ref().map(|boot| {
        assert!(boot.boot_time_ms > 0);
    });
}
