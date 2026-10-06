use cstyle::api::format_bytes;
use cstyle::config::FormatOptions;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

fn performance_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("perf lock")
}

fn format_ok(input: &str) {
    let output = format_bytes(input.as_bytes(), &FormatOptions::default()).expect("format bytes");
    assert!(!output.is_empty());
}

#[test]
fn deeply_nested_parentheses_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 30_000;
    let input = format!("int x = {}1{};\n", "(".repeat(n), ")".repeat(n));
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "nested parens formatting took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn many_sequential_preprocessor_guards_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 30_000;
    let mut input = String::with_capacity(n * 24);
    for i in 0..n {
        input.push_str(&format!("#if A{i}\nint v{i};\n#endif\n"));
    }
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "sequential preprocessor guards took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn long_open_brace_run_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 30_000;
    let input = "{".repeat(n);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "open brace run formatting took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn deeply_nested_blocks_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 1_500;
    let mut input = String::with_capacity(n * 24);
    for i in 0..n {
        input.push_str(&format!("if (a{i}) {{\n"));
    }
    input.push_str("b();\n");
    for _ in 0..n {
        input.push_str("}\n");
    }
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "deeply nested blocks took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn long_binary_expression_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 30_000;
    let terms = vec!["a"; n].join(" + ");
    let input = format!("int x = {terms};\n");
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "long expression formatting took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn long_ternary_question_chain_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = "a?".repeat(40_000);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "ternary question chain took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn unclosed_preprocessor_conditional_pile_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 30_000;
    let mut input = String::with_capacity(n * 8);
    for i in 0..n {
        input.push_str(&format!("#if A{i}\n"));
    }
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "unclosed preprocessor pile took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn unterminated_string_literal_pile_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = "\"abc\n".repeat(10_000);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "unterminated string pile took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn repeated_if_headers_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = "if(a) ".repeat(30_000);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "repeated if headers took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn open_paren_pile_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = "(".repeat(150_000);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "open paren pile took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn deep_preprocessor_split_else_chain_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let depth = 512;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
    }
    input.push_str(
        "  if(t){\n    switch(value){\n      case ONE: {\n        if(value){\n          call(stderr,\n            \"message\\n\"\n          );\n          rc = 1;\n        }\n        break;\n      }\n    }\n  }\n}\n",
    );
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 2.5,
        "deep split-else chain took {elapsed:?}, expected bounded runtime (< 2.5s)"
    );
}

#[test]
fn repeated_open_struct_heads_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = "struct A{\n".repeat(2_500);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "repeated open struct heads took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn mixed_template_struct_malformed_input_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = "template<class T> struct A{ if(x) ".repeat(3_000);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "mixed malformed template struct input took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn long_multi_line_expression_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = format!("int x = a\n{};\n", "    + a\n".repeat(30_000));
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "multi-line expression took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn many_parenthesized_ternary_arms_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = format!(
        "void f() {{\n{}}}\n",
        "    x = f(a ?\n          b : c);\n".repeat(20_000)
    );
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "parenthesized ternary arms took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn long_line_split_at_maximum_length_stays_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let terms = vec!["a"; 30_000].join(" + ");
    let input = format!("int x = {terms};\n");
    let mut options = FormatOptions::default();
    options.max_code_length = Some(100);
    let start = Instant::now();
    let output = format_bytes(input.as_bytes(), &options).expect("format bytes");
    let elapsed = start.elapsed();
    assert!(output.len() > input.len());
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "splitting a long line took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn braceless_headers_nested_past_the_stack_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let input = "if(a) ".repeat(60_000);
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "deeply nested headers took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn nested_ternary_colons_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 20_000;
    let input = format!("x = {}1{};\n", "a ? ".repeat(n), " : 2".repeat(n));
    let start = Instant::now();
    format_ok(&input);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "nested ternary colons took {elapsed:?}, expected bounded runtime (< 5s)"
    );
}

#[test]
fn nested_calls_and_subscripts_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 50_000;
    for input in [
        format!("x = {}{};\n", "f(".repeat(n), ")".repeat(n)),
        format!("x = a{}{};\n", "[a".repeat(n), "]".repeat(n)),
    ] {
        let start = Instant::now();
        format_ok(&input);
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs_f64() < 5.0,
            "nested calls or subscripts took {elapsed:?}, expected bounded runtime (< 5s)"
        );
    }
}

#[test]
fn long_bodies_that_look_back_to_their_opener_stay_bounded() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = performance_lock();
    let n = 60_000;
    let statements = |indent: &str| {
        (0..n)
            .map(|i| format!("{indent}a{i} = b{i} + c;\n"))
            .collect::<String>()
    };
    let calls = (0..n / 2)
        .map(|i| format!("    f{i}(\n        a);\n"))
        .collect::<String>();
    let entries = (0..n)
        .map(|i| format!("    ON_COMMAND(ID_{i}, f{i})\n"))
        .collect::<String>();
    for (shape, input) in [
        (
            "a label block",
            format!(
                "void f(int x)\n{{\n    switch (x) {{\n    default: {{\n        break;\n    }}\n    }}\n{}}}\n",
                statements("    ")
            ),
        ),
        (
            "a case body split by a directive",
            format!(
                "void f(int x)\n{{\n    switch (x) {{\n    case 1:\n#if A\n        if (x)\n#else\n        if (!x)\n#endif\n{}    }}\n}}\n",
                statements("        ")
            ),
        ),
        (
            "calls split after their paren",
            format!("void f(void)\n{{\n{calls}}}\n"),
        ),
        (
            "a macro block of calls",
            format!("BEGIN_MESSAGE_MAP(A, B)\n{entries}END_MESSAGE_MAP()\n"),
        ),
    ] {
        let start = Instant::now();
        format_ok(&input);
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs_f64() < 5.0,
            "{shape} took {elapsed:?}, expected bounded runtime (< 5s)"
        );
    }
}
