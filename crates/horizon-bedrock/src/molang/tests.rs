use super::*;
use std::collections::HashMap;

#[derive(Default)]
struct TestHost {
    queries: HashMap<String, f32>,
    arrays: HashMap<String, Vec<Value>>,
    other: HashMap<(u64, String), f32>,
    calls: Vec<(String, Vec<Value>)>,
}

impl Host for TestHost {
    fn query(&mut self, name: &str, args: &[Value]) -> Value {
        self.calls.push((name.to_owned(), args.to_vec()));
        match self.queries.get(name) {
            Some(n) => Value::Num(*n),
            None => Value::Null,
        }
    }
    fn context(&mut self, name: &str) -> Value {
        match name {
            "item_slot" => "main_hand".into(),
            "owning_entity" => Value::Entity(7),
            _ => Value::Null,
        }
    }
    fn array(&self, name: &str) -> Option<Vec<Value>> {
        self.arrays.get(name).cloned()
    }
    fn entity_var(&mut self, entity: u64, var: Sym) -> Value {
        match self.other.get(&(entity, sym_name(var).to_string())) {
            Some(n) => Value::Num(*n),
            None => Value::Null,
        }
    }
}

fn run_with(src: &str, host: &mut TestHost, vars: &mut Vars) -> Value {
    let program = parse(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    let mut rng = Rng::new(1);
    program.eval(&mut EvalCtx {
        host,
        vars,
        rng: &mut rng,
        this: Value::Num(5.0),
    })
}

fn run(src: &str) -> f32 {
    run_with(src, &mut TestHost::default(), &mut Vars::default()).num()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(run("1 + 2 * 3"), 7.0);
    assert_eq!(run("(1 + 2) * 3"), 9.0);
    assert_eq!(run("10 - 4 - 3"), 3.0);
    assert_eq!(run("-2 * -3"), 6.0);
    assert_eq!(run("1 + 2 < 4 && 5 > 4"), 1.0);
    assert_eq!(run("1 == 1 || 0"), 1.0);
    assert_eq!(run("!0 + !1"), 1.0);
    assert_eq!(run(".5 + 1.5"), 2.0);
    assert_eq!(run("1.0f + 1"), 2.0);
    assert_eq!(run("2e1"), 20.0);
}

#[test]
fn division_by_zero_is_zero() {
    assert_eq!(run("1 / 0"), 0.0);
    assert_eq!(run("math.mod(5, 0)"), 0.0);
    assert_eq!(run("math.sqrt(-1)"), 0.0);
}

#[test]
fn ternaries() {
    assert_eq!(run("1 ? 2 : 3"), 2.0);
    assert_eq!(run("0 ? 2 : 3"), 3.0);
    assert_eq!(run("0 ? 2"), 0.0);
    assert_eq!(run("1 ? 2"), 2.0);
    assert_eq!(run("0 ? 1 : 0 ? 2 : 3"), 3.0);
    assert_eq!(run("1 ? 0 ? 5 : 6 : 7"), 6.0);
}

#[test]
fn variables_persist_and_temps_do_not() {
    let mut host = TestHost::default();
    let mut vars = Vars::default();
    run_with("v.count = v.count + 1; t.x = 4;", &mut host, &mut vars);
    run_with("variable.count = variable.count + 1", &mut host, &mut vars);
    assert_eq!(vars[&intern("count")], Value::Num(2.0));
    assert_eq!(run_with("t.x", &mut host, &mut vars), Value::Null);
    assert_eq!(run("t.x = 3; t.x * 2"), 0.0);
    assert_eq!(run("t.x = 3; return t.x * 2;"), 6.0);
}

#[test]
fn dotted_variable_names() {
    let mut vars = Vars::default();
    run_with("v.a.b = 3;", &mut TestHost::default(), &mut vars);
    assert_eq!(vars[&intern("a.b")], Value::Num(3.0));
}

#[test]
fn coalesce() {
    assert_eq!(run("v.nothing ?? 4"), 4.0);
    assert_eq!(run("-0.9 - (v.nothing ?? 0)"), -0.9);
    assert_eq!(run("0 ?? 4"), 0.0);
    assert_eq!(run("q.unknown ?? 9"), 9.0);
    assert_eq!(run("(v.nothing ?? 2) + 1"), 3.0);
}

#[test]
fn blocks_return_and_statements() {
    assert_eq!(run("1; 2"), 0.0);
    assert_eq!(run("return 5; 6"), 5.0);
    assert_eq!(run("1 ? { t.a = 2; return t.a + 1; }; 9"), 3.0);
    assert_eq!(run("0 ? { return 1; }; return 8;"), 8.0);
    assert_eq!(run("v.q ? { v.z = 1; } : { v.z = 2; }; return v.z;"), 2.0);
}

#[test]
fn loops() {
    assert_eq!(
        run("t.s = 0; loop(5, { t.s = t.s + 2; }); return t.s;"),
        10.0
    );
    assert_eq!(
        run("t.s = 0; loop(100, { t.s = t.s + 1; t.s >= 3 ? { break; }; }); return t.s;"),
        3.0
    );
    assert_eq!(
        run(
            "t.s = 0; t.i = 0; loop(6, { t.i = t.i + 1; math.mod(t.i, 2) == 0 ? { continue; }; t.s = t.s + t.i; }); return t.s;"
        ),
        9.0
    );
    assert_eq!(
        run("t.s = 0; loop(100000, { t.s = t.s + 1; }); return t.s;"),
        1024.0
    );
    assert_eq!(run("t.s = 7; loop(-1, { t.s = 0; }); return t.s;"), 7.0);
}

#[test]
fn runaway_expression_gives_zero() {
    let src =
        "t.s = 0; loop(1024, { loop(1024, { loop(1024, { t.s = t.s + 1; }); }); }); return 1;";
    assert_eq!(run(src), 0.0);
}

#[test]
fn for_each_walks_the_array() {
    let mut host = TestHost::default();
    host.arrays.insert(
        "xs".into(),
        vec![Value::Num(1.0), Value::Num(2.0), Value::Num(4.0)],
    );
    let v = run_with(
        "t.s = 0; for_each(t.x, array.xs, { t.s = t.s + t.x; }); return t.s;",
        &mut host,
        &mut Vars::default(),
    );
    assert_eq!(v, Value::Num(7.0));
}

#[test]
fn array_index_clamps_below_and_wraps_above() {
    let mut host = TestHost::default();
    host.arrays
        .insert("skins".into(), vec!["a".into(), "b".into(), "c".into()]);
    let mut vars = Vars::default();
    assert_eq!(run_with("array.skins[1]", &mut host, &mut vars), "b".into());
    assert_eq!(run_with("Array.Skins[4]", &mut host, &mut vars), "b".into());
    assert_eq!(
        run_with("array.skins[-3]", &mut host, &mut vars),
        "a".into()
    );
    assert_eq!(
        run_with("array.skins[2.9]", &mut host, &mut vars),
        "c".into()
    );
    assert_eq!(run_with("array.nope[0]", &mut host, &mut vars), Value::Null);
}

#[test]
fn strings_and_context() {
    assert_eq!(run("c.item_slot == 'main_hand'"), 1.0);
    assert_eq!(run("c.item_slot == 'off_hand'"), 0.0);
    assert_eq!(run("c.item_slot != 'off_hand'"), 1.0);
    assert_eq!(run("'a' == 'A'"), 0.0);
    assert_eq!(run("context.Item_Slot == 'main_hand' ? 3 : 4"), 3.0);
}

#[test]
fn queries_get_their_arguments() {
    let mut host = TestHost::default();
    host.queries.insert("is_item_name_any".into(), 1.0);
    let mut vars = Vars::default();
    let v = run_with(
        "q.is_item_name_any('slot.weapon.mainhand', 'minecraft:stick') ? 10 : 20",
        &mut host,
        &mut vars,
    );
    assert_eq!(v, Value::Num(10.0));
    assert_eq!(host.calls[0].0, "is_item_name_any");
    assert_eq!(
        host.calls[0].1,
        vec![
            Value::from("slot.weapon.mainhand"),
            Value::from("minecraft:stick")
        ]
    );
    run_with("Query.Anim_Time + query.life_time", &mut host, &mut vars);
    assert_eq!(host.calls[1].0, "anim_time");
    assert_eq!(host.calls[2].0, "life_time");
}

#[test]
fn arrow_reads_another_entitys_variable() {
    let mut host = TestHost::default();
    host.other.insert((7, "mode".into()), 3.0);
    let mut vars = Vars::default();
    assert_eq!(
        run_with("c.owning_entity -> v.mode", &mut host, &mut vars),
        Value::Num(3.0)
    );
    run_with("v.copy = c.owning_entity -> v.mode;", &mut host, &mut vars);
    assert_eq!(vars[&intern("copy")], Value::Num(3.0));
    assert_eq!(
        run_with("v.not_an_entity -> v.mode", &mut host, &mut vars),
        Value::Null
    );
    assert_eq!(
        run_with(
            "q.is_attached ? c.owning_entity -> v.mode;",
            &mut host,
            &mut vars
        ),
        Value::Num(0.0)
    );
}

#[test]
fn resources() {
    let v = run_with(
        "v.a ? Texture.One : Texture.Two",
        &mut TestHost::default(),
        &mut Vars::default(),
    );
    assert_eq!(v, Value::Res(ResKind::Texture, "two".into()));
    let v = run_with(
        "geometry.default",
        &mut TestHost::default(),
        &mut Vars::default(),
    );
    assert_eq!(v, Value::Res(ResKind::Geometry, "default".into()));
}

#[test]
fn this_is_the_channels_value() {
    assert_eq!(run("this * 2"), 10.0);
}

#[test]
fn math_functions_use_degrees() {
    assert!(close(run("math.sin(90)"), 1.0));
    assert!(close(run("math.cos(180)"), -1.0));
    assert!(close(run("math.atan2(1, 1)"), 45.0));
    assert!(close(run("math.asin(1)"), 90.0));
    assert!(close(run("math.pi"), std::f32::consts::PI));
    assert_eq!(run("math.clamp(5, 0, 3)"), 3.0);
    assert_eq!(run("math.lerp(10, 20, 0.25)"), 12.5);
    assert_eq!(run("math.min(2, 3) + math.max(2, 3)"), 5.0);
    assert_eq!(
        run("math.abs(-3) + math.floor(2.7) + math.ceil(2.1) + math.trunc(-2.7)"),
        3.0 + 2.0 + 3.0 - 2.0
    );
    assert_eq!(run("math.round(2.5)"), 3.0);
    assert_eq!(run("math.mod(7, 3)"), 1.0);
    assert_eq!(run("math.pow(2, 10)"), 1024.0);
    assert_eq!(run("math.sign(-4)"), -1.0);
    assert_eq!(run("math.copy_sign(3, -1)"), -3.0);
    assert_eq!(run("math.lerprotate(350, 10, 0.5)"), 360.0);
    assert_eq!(run("math.min_angle(270)"), -90.0);
    assert!(close(run("math.hermite_blend(0.5)"), 0.5));
}

#[test]
fn inverse_lerp_booleans_and_assignment_in_parentheses() {
    assert!(close(run("math.inverse_lerp(0, 15, 3)"), 0.2));
    assert_eq!(run("math.inverse_lerp(1, 1, 3)"), 0.0);
    assert_eq!(run("true + true + false"), 2.0);
    assert_eq!(run("1 ? (t.a = 4); return t.a;"), 4.0);
    assert_eq!(
        run("t.a = 2; 1 ? ( t.a = math.random( -5, 5 ) ); return t.a != 2 || 1;"),
        1.0
    );
}

#[test]
fn easing_curves() {
    for name in [
        "sine", "quad", "cubic", "quart", "quint", "expo", "circ", "back", "elastic", "bounce",
    ] {
        for mode in ["in", "out", "in_out"] {
            let f = |t: &str| run(&format!("math.ease_{mode}_{name}(10, 20, {t})"));
            assert!(close(f("0"), 10.0), "{mode} {name} 0");
            assert!(close(f("1"), 20.0), "{mode} {name} 1");
        }
    }
    assert!(close(run("math.ease_in_quad(0, 1, 0.5)"), 0.25));
    assert!(close(run("math.ease_out_quad(0, 1, 0.5)"), 0.75));
    assert!(close(run("math.ease_in_out_cubic(0, 1, 0.5)"), 0.5));
    assert!(close(run("math.ease_in_out_sine(0, 1, 0.5)"), 0.5));
}

#[test]
fn randoms_stay_in_range_and_repeat_for_a_seed() {
    for _ in 0..50 {
        let v = run("math.random(2, 3)");
        assert!((2.0..3.0).contains(&v));
        let i = run("math.random_integer(1, 6)");
        assert!((1.0..=6.0).contains(&i) && i.fract() == 0.0);
    }
    let a = run("math.random(0, 1000)");
    let b = run("math.random(0, 1000)");
    assert_eq!(a, b);
    let d = run("math.die_roll_integer(3, 1, 1)");
    assert_eq!(d, 3.0);
}

#[test]
fn constants() {
    assert_eq!(parse("1.5").unwrap().constant(), Some(1.5));
    assert_eq!(parse("-2").unwrap().constant(), Some(-2.0));
    assert_eq!(parse("q.x").unwrap().constant(), None);
    assert_eq!(parse("1; 2").unwrap().constant(), None);
}

#[test]
fn parse_errors_say_where() {
    for src in [
        "1 +",
        "(1",
        "v.a = ",
        "q.",
        "math.nope(1)",
        "'open",
        "1 2",
        "loop(1)",
        "foo.bar",
        "1 = 2",
        "x",
    ] {
        assert!(parse(src).is_err(), "{src} should not parse");
    }
    let e = parse("1 + @").unwrap_err();
    assert_eq!(e.at, 4);
}

#[test]
fn shapes_seen_in_packs() {
    // Statements and nested ternaries with blocks, as packs write them.
    let src = "v.a = q.life_time; v.a >= 3 ? { v.b ? { v.c = 1; } : { v.c = 2; }; v.a = 0; };";
    let mut host = TestHost::default();
    host.queries.insert("life_time".into(), 4.0);
    let mut vars = Vars::default();
    run_with(src, &mut host, &mut vars);
    assert_eq!(vars[&intern("c")], Value::Num(2.0));
    assert_eq!(vars[&intern("a")], Value::Num(0.0));
    // Skinned arrays chained with ternaries and an index sum.
    assert!(parse("v.x ? Texture.a : v.y ? Array.skins[v.i + 7] : Array.skins[v.i]").is_ok());
    // Trailing and repeated semicolons, newlines.
    assert!(parse("\n v.a = 1;;\n v.b = 2;\n").is_ok());
    // Comparison of negated query.
    assert!(parse("!q.is_owner_identifier_any('minecraft:player', 'minecraft:armor_stand') ? { v.a = 1; };").is_ok());
}

#[test]
fn not_binds_tighter_than_the_conditional() {
    assert_eq!(run("v.a=0; v.b=!v.a?1:2; return v.b;"), 1.0);
    assert_eq!(run("v.a=1; v.b=!v.a?1:2; return v.b;"), 2.0);
    assert_eq!(
        run("v.d=!v.zz?1:(v.k?0:Math.clamp(5/0.2,0,1)); return v.d;"),
        1.0
    );
}

#[test]
fn any_all_and_approx_eq_are_functions_of_their_arguments() {
    let mut host = TestHost::default();
    let mut vars = Vars::default();
    let mut run = |src: &str| run_with(src, &mut host, &mut vars).num();
    assert_eq!(run("q.any(5, 1, 5, 9)"), 1.0);
    assert_eq!(run("q.any(4, 1, 5, 9)"), 0.0);
    assert_eq!(run("q.all(2, 2, 2)"), 1.0);
    assert_eq!(run("q.all(2, 2, 3)"), 0.0);
    assert_eq!(run("q.approx_eq(1, 1.0000001)"), 1.0);
}

#[test]
fn a_function_without_a_result_gives_zero_not_nan() {
    assert_eq!(run("return Math.asin(2);"), 0.0);
    assert_eq!(
        run("v.a=Math.lerp(0, Math.clamp(-Math.asin(3/1), -90, 90), 0.5); return v.a;"),
        0.0
    );
    assert_eq!(run("return Math.sqrt(-4);"), 0.0);
}

#[test]
fn a_parenthesis_holds_a_statement_list_worth_its_last_one() {
    assert!(close(run("( v.a = 4; v.b = 3; v.a * v.b; )"), 12.0));
    assert!(close(run("-( v.a = 2; v.a + 1; ) + 10"), 7.0));
    assert!(close(run("(1)"), 1.0));
}
