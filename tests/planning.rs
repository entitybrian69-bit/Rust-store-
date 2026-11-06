use quick_place::{facing_from_yaw, plan, Mode, Pos};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture { case: Vec<Case> }
#[derive(Deserialize)]
struct Case {
    name: String, yaw: f32, mode: String, limit: u32,
    origin: [i32; 3], blocked: Vec<[i32; 3]>, expected: Vec<[i32; 3]>,
}
fn pos(p: [i32; 3]) -> Pos { Pos { x: p[0], y: p[1], z: p[2] } }

#[test]
fn toml_fixtures_exercise_planning() {
    let fixture: Fixture = toml::from_str(include_str!("fixtures/planning.toml")).unwrap();
    for c in fixture.case {
        let mode = match c.mode.as_str() { "line" => Mode::Line, "plane" => Mode::Plane, _ => panic!("invalid mode") };
        let got = plan(pos(c.origin), facing_from_yaw(c.yaw).unwrap(), mode, c.limit,
            |p| !c.blocked.iter().any(|b| pos(*b) == p));
        assert_eq!(got, c.expected.into_iter().map(pos).collect::<Vec<_>>(), "{}", c.name);
    }
}

#[test]
fn directions_and_bounds() {
    use quick_place::Facing::*;
    for (angle, facing) in [(0., South), (44.9, South), (45., West), (135., North), (225., East), (315., South), (-90., East), (360., South)] {
        assert_eq!(facing_from_yaw(angle), Some(facing));
    }
    assert_eq!(facing_from_yaw(f32::NAN), None);
    assert_eq!(facing_from_yaw(f32::INFINITY), None);
    let origin = Pos { x: 0, y: 0, z: 0 };
    assert_eq!(plan(origin, South, Mode::Line, 0, |_| true).len(), 0);
    assert_eq!(plan(origin, South, Mode::Line, u32::MAX, |_| true).len(), 64);
}
