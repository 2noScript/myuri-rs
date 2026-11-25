use myuri_rs::cursory::{generate_trajectory, TrajectoryOptions};
use myuri_rs::math::{BoundingBox, Vector};
use myuri_rs::plan::{
    plan_click, plan_move, plan_point, plan_select_all, plan_type, plan_wheel, PlanMoveOptions,
    PlanTypeOptions, PlanWheelOptions, PlannedWheelStep,
};
use myuri_rs::profile::generate_profile;
use myuri_rs::rng::make_rng;
use myuri_rs::settings::{resolve_settings_value, ScrollDevice, WheelStyle};
use myuri_rs::target_os::Os;
use myuri_rs::typing::KeyActionType;
use myuri_rs::MyuriError;
use serde_json::json;

#[test]
fn cursory_trajectory_matches_cursory_js_golden() {
    let traj = generate_trajectory(
        [100.0, 120.0],
        [640.0, 480.0],
        TrajectoryOptions {
            frequency: 100.0,
            seed: Some(1234),
            ..Default::default()
        },
    );
    assert_eq!(traj.points.len(), 87);
    assert_eq!(traj.timings[..5], [0.0, 10.0, 20.0, 30.0, 41.0]);
    assert_eq!(traj.points[0], [100.0, 120.0]);
    assert!((traj.points[1][0] - 99.93322184603839).abs() < 1e-9);
    assert!((traj.points[1][1] - 120.7741789058808).abs() < 1e-9);
    assert_eq!(*traj.points.last().unwrap(), [640.0, 480.0]);
}

#[test]
fn generate_profile_matches_myuri_ts_golden() {
    let p = generate_profile(1234, None, Some(Os::Macos));
    assert!((p.mouse.speed_factor - 0.9340308850494072).abs() < 1e-12);
    assert!((p.overshoot.radius - 125.33245108538128).abs() < 1e-12);
    assert!((p.overshoot.threshold - 593.3437940231134).abs() < 1e-12);
    assert!((p.overshoot.correction_speed_factor - 1.062116276958504).abs() < 1e-12);
    assert!((p.overshoot.dwell.min_ms - 21.887015941619325).abs() < 1e-12);
    assert!((p.overshoot.dwell.max_ms - 57.22369931537004).abs() < 1e-12);
    assert!((p.scroll.speed_factor - 0.989651989431932).abs() < 1e-12);
    assert!((p.scroll.finish_position - 0.4296680652307818).abs() < 1e-12);
    assert!((p.scroll.realism.speed_irregularity - 0.08793830619581952).abs() < 1e-12);
    assert_eq!(p.scroll.device, ScrollDevice::Trackpad);
    assert_eq!(p.scroll.wheel.style, WheelStyle::Macos);
    assert!((p.typing.wpm - 81.6000448882113).abs() < 1e-12);
    assert!((p.typing.mistake_multiplier - 1.1169158843923896).abs() < 1e-12);
    assert!((p.typing.pauses.rate - 1.0032505031734065).abs() < 1e-12);
    assert!((p.typing.pauses.length - 1.0314695042693232).abs() < 1e-12);
}

#[test]
fn plan_move_matches_myuri_ts_golden() {
    let rng = make_rng(42);
    let pts = plan_move(
        Vector::new(50.0, 80.0),
        Vector::new(700.0, 500.0),
        &rng,
        &PlanMoveOptions {
            os: Some(Os::Windows),
            ..Default::default()
        },
    );
    assert_eq!(pts.len(), 65);
    assert!((pts[0].x - 51.03719838183991).abs() < 1e-9);
    assert!((pts[0].y - 85.90546931235204).abs() < 1e-9);
    assert!((pts[0].t - 10.3820342292255).abs() < 1e-9);
    let last = pts.last().unwrap();
    assert_eq!(last.x, 700.0);
    assert_eq!(last.y, 500.0);
    assert!((last.t - 709.9555233358549).abs() < 1e-9);
}

#[test]
fn plan_click_matches_myuri_ts_golden() {
    let rng = make_rng(42);
    let c = plan_click(&rng, 2, None);
    assert_eq!(c.dwell_ms, 11.0);
    assert_eq!(c.presses, vec![(0.0, 88.0), (151.0, 215.0)]);
}

#[test]
fn plan_type_and_select_all_match_myuri_ts_golden() {
    let steps = plan_type(
        "Hello, Camoufox! 2026",
        &make_rng(42),
        &PlanTypeOptions {
            os: Some(Os::Macos),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(steps.len(), 68);
    assert_eq!(steps[0].at, 0.0);
    assert_eq!(steps[0].action, KeyActionType::Down);
    assert_eq!(steps[0].key, "Shift");
    assert!((steps[1].at - 397.84319204493715).abs() < 1e-9);
    assert_eq!(steps[1].action, KeyActionType::Down);
    assert_eq!(steps[1].key, "H");
    let last = steps.last().unwrap();
    assert!((last.at - 7250.570609957435).abs() < 1e-9);
    assert_eq!(last.action, KeyActionType::Up);
    assert_eq!(last.key, "6");

    let sel = plan_select_all(
        &make_rng(42),
        Some(Os::Macos),
        &PlanTypeOptions::default(),
    );
    assert_eq!(sel.len(), 4);
    assert_eq!(sel[0].key, "Meta");
    assert_eq!(sel[1].key, "a");
    assert!((sel[1].at - 38.09015376973863).abs() < 1e-9);
    assert!((sel[2].at - 73.81273919997724).abs() < 1e-9);
    assert_eq!(sel[2].key, "Meta");
    assert_eq!(sel[2].action, KeyActionType::Up);
    assert!((sel[3].at - 107.34292198005178).abs() < 1e-9);
}

#[test]
fn plan_wheel_matches_myuri_ts_golden() {
    let mouse_steps = plan_wheel(
        0.0,
        600.0,
        &make_rng(42),
        &PlanWheelOptions {
            os: Some(Os::Linux),
            device: Some(ScrollDevice::Mouse),
            settings: None,
        },
    );
    assert_eq!(mouse_steps.len(), 6);
    assert_eq!(
        mouse_steps[..4],
        [
            PlannedWheelStep::Wheel { dx: 0.0, dy: 100.0, at: 0.0 },
            PlannedWheelStep::Wheel { dx: 0.0, dy: 100.0, at: 66.0 },
            PlannedWheelStep::Wheel { dx: 0.0, dy: 100.0, at: 133.0 },
            PlannedWheelStep::Wheel { dx: 0.0, dy: 100.0, at: 150.0 },
        ]
    );

    let tp_steps = plan_wheel(
        120.0,
        400.0,
        &make_rng(42),
        &PlanWheelOptions {
            os: Some(Os::Macos),
            device: Some(ScrollDevice::Trackpad),
            settings: None,
        },
    );
    assert_eq!(tp_steps.len(), 37);
    assert_eq!(
        tp_steps[..4],
        [
            PlannedWheelStep::Wheel { dx: 0.0, dy: 8.0, at: 7.0 },
            PlannedWheelStep::Wheel { dx: 0.0, dy: 8.0, at: 14.0 },
            PlannedWheelStep::Wheel { dx: 0.0, dy: 11.0, at: 23.0 },
            PlannedWheelStep::Wheel { dx: 0.0, dy: 15.0, at: 31.0 },
        ]
    );
}

#[test]
fn resolve_settings_value_rejects_unknown_keys() {
    let err = resolve_settings_value(Some(&json!({ "mouse": { "bogus": 1 } }))).unwrap_err();
    assert!(matches!(err, MyuriError::UnknownSetting(k) if k == "mouse.bogus"));
}

#[test]
fn plan_point_keeps_start_if_inside_box() {
    let box_rect = BoundingBox {
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 40.0,
    };
    let inside = Vector::new(120.0, 115.0);
    let chosen = plan_point(box_rect, &make_rng(7), Some(inside)).unwrap();
    assert_eq!(chosen, inside);
}
