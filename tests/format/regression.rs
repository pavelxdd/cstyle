use crate::common::format;
use cstyle::api::format_bytes;
use cstyle::config::{FormatOptions, apply_command_line_args};

fn non_whitespace(source: &str) -> String {
    source.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn check(input: &str, args: &[&str], expected: &str) {
    let mut options = FormatOptions::default();
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    apply_command_line_args(&mut options, &args).expect("valid options");
    let output = format_bytes(input.as_bytes(), &options).expect("format bytes");
    assert_eq!(String::from_utf8(output).expect("utf8"), expected);
}

#[test]
fn struct_members_after_continued_preprocessor_condition_keep_indent() {
    check(
        "#ifdef FEATURE\n#if defined(ALPHA) && \\\n    !defined(BETA)\n\n#include <item.h>\ntypedef struct {\n    flag_t first;\n    int second;\n} Item;\n#endif\n#endif\n",
        &[],
        "#ifdef FEATURE\n#if defined(ALPHA) && \\\n    !defined(BETA)\n\n#include <item.h>\ntypedef struct {\n    flag_t first;\n    int second;\n} Item;\n#endif\n#endif\n",
    );
}

#[test]
fn top_level_function_after_typedef_struct_stays_unindented() {
    check(
        "typedef struct {\n  int value;\n} Item;\n\n/* comment */\nstatic int f(void) {\n  return 0;\n}\n",
        &[],
        "typedef struct {\n    int value;\n} Item;\n\n/* comment */\nstatic int f(void) {\n    return 0;\n}\n",
    );
}

#[test]
fn top_level_function_after_question_mark_define_stays_unindented() {
    check(
        "#define SPECIALS \"^$*+?.([%-\"\n\nstatic int f(void) {\n  return 0;\n}\n",
        &[],
        "#define SPECIALS \"^$*+?.([%-\"\n\nstatic int f(void) {\n    return 0;\n}\n",
    );
}

#[test]
fn function_after_multiline_call_keeps_return_after_switch_indent() {
    check(
        "static int previous(void) {\n  if( rc || error ){\n    call(out,\n            \"text: %d\\n\", value);\n  }else if( flag ){\n    call(out,\n            \"changes: %lld\\n\",\n            count());\n  }\n  if( done() ) return 1;\n  return 0;\n}\n\nstatic int call(void) {\n  int rc = 0;\n  if( state<7 ) {\n    switch( state ) {\n    case 0: {\n      if( safe==0\n       && length(value)>=24\n       && match(value, expect)==0\n      ){\n        state = 1;\n      }else{\n        state = 7;\n      }\n      break;\n    };\n    }\n  }\n\n  return rc;\n}\n",
        &[],
        "static int previous(void) {\n    if( rc || error ) {\n        call(out,\n             \"text: %d\\n\", value);\n    } else if( flag ) {\n        call(out,\n             \"changes: %lld\\n\",\n             count());\n    }\n    if( done() ) return 1;\n    return 0;\n}\n\nstatic int call(void) {\n    int rc = 0;\n    if( state<7 ) {\n        switch( state ) {\n        case 0: {\n            if( safe==0\n                    && length(value)>=24\n                    && match(value, expect)==0\n              ) {\n                state = 1;\n            } else {\n                state = 7;\n            }\n            break;\n        };\n        }\n    }\n\n    return rc;\n}\n",
    );
}

#[test]
fn split_function_header_after_preprocessor_close_keeps_closing_brace_unindented() {
    check(
        "static char *lookup(const char *env, const char *sub,\n                           const char *name){\n#if defined(WIN32) || defined(WIN64) \\\n     || defined(OTHER)\n  return 0;\n#else\n  char *result = 0;\n  if( env ){\n    result = make(\"%s/%s\", env, name);\n  }\n  return result;\n#endif\n}\n",
        &[],
        "static char *lookup(const char *env, const char *sub,\n                    const char *name) {\n#if defined(WIN32) || defined(WIN64) \\\n     || defined(OTHER)\n    return 0;\n#else\n    char *result = 0;\n    if( env ) {\n        result = make(\"%s/%s\", env, name);\n    }\n    return result;\n#endif\n}\n",
    );
}

#[test]
fn fragment_after_preprocessor_close_keeps_following_function_close_indent() {
    check(
        "#endif\n\n  if( value ){\n    int n = size(value) + 1;\n    char *copy = alloc(n);\n    if( copy ) save(copy, value, n);\n    value = copy;\n  }\n\n  return value;\n}\n\nstatic char *lookup(const char *env, const char *sub, const char *name){\n#if defined(WIN32)\n  return 0;\n#else\n  char *result = 0;\n  const char *dir;\n\n  dir = env ? getenv(env) : 0;\n  if( dir ){\n    result = make(\"%s/%s\", dir, name);\n  }else{\n    const char *home = find_home();\n    if( home==0 ) return 0;\n    result = (sub && *sub)\n      ? make(\"%s/%s/%s\", home, sub, name)\n      : make(\"%s/%s\", home, name);\n  }\n  check(result);\n  if( access(result,0)!=0 ){\n    free(result);\n    result = 0;\n  }\n  return result;\n#endif\n}\n",
        &[],
        "#endif\n\nif( value ) {\n    int n = size(value) + 1;\n    char *copy = alloc(n);\n    if( copy ) save(copy, value, n);\n    value = copy;\n}\n\nreturn value;\n}\n\nstatic char *lookup(const char *env, const char *sub, const char *name) {\n#if defined(WIN32)\n    return 0;\n#else\n    char *result = 0;\n    const char *dir;\n\n    dir = env ? getenv(env) : 0;\n    if( dir ) {\n        result = make(\"%s/%s\", dir, name);\n    } else {\n        const char *home = find_home();\n        if( home==0 ) return 0;\n        result = (sub && *sub)\n                 ? make(\"%s/%s/%s\", home, sub, name)\n                 : make(\"%s/%s\", home, name);\n    }\n    check(result);\n    if( access(result,0)!=0 ) {\n        free(result);\n        result = 0;\n    }\n    return result;\n#endif\n}\n",
    );
}

#[test]
fn function_after_preprocessor_close_keeps_top_level_closing_brace() {
    check(
        "#endif\n\nint previous(void){\n  return 0;\n}\n\nstatic char *lookup(const char *env, const char *sub, const char *name){\n#if defined(WIN32)\n  return 0;\n#else\n  char *result = 0;\n  const char *dir;\n\n  dir = env ? getenv(env) : 0;\n  if( dir ){\n    result = make(\"%s/%s\", dir, name);\n  }else{\n    const char *home = find_home();\n    if( home==0 ) return 0;\n    result = (sub && *sub)\n      ? make(\"%s/%s/%s\", home, sub, name)\n      : make(\"%s/%s\", home, name);\n  }\n  check(result);\n  if( access(result,0)!=0 ){\n    free(result);\n    result = 0;\n  }\n  return result;\n#endif\n}\n",
        &[],
        "#endif\n\nint previous(void) {\n    return 0;\n}\n\nstatic char *lookup(const char *env, const char *sub, const char *name) {\n#if defined(WIN32)\n    return 0;\n#else\n    char *result = 0;\n    const char *dir;\n\n    dir = env ? getenv(env) : 0;\n    if( dir ) {\n        result = make(\"%s/%s\", dir, name);\n    } else {\n        const char *home = find_home();\n        if( home==0 ) return 0;\n        result = (sub && *sub)\n                 ? make(\"%s/%s/%s\", home, sub, name)\n                 : make(\"%s/%s\", home, name);\n    }\n    check(result);\n    if( access(result,0)!=0 ) {\n        free(result);\n        result = 0;\n    }\n    return result;\n#endif\n}\n",
    );
}

#[test]
fn switch_case_after_split_else_chain_keeps_case_continuation_indent() {
    check(
        "static int previous(int c){\n  if( c==0 ){\n    call(out, \"text %d\\n\", c);\n  }else\n\n  if( c==99 ){\n    call(out, \"text %d\\n\", c);\n  }else\n\n  {\n    return 1;\n  }\n  return 0;\n}\n\nstatic int update(Item *item, const char *text){\n  int rc = OK;\n\n  if( item->state<7 ){\n    switch( item->state ){\n      case 0: {\n        const char *expect = \"alpha\";\n        assert( length(expect)==5 );\n        if( item->safe==0\n         && length(text)>=5\n         && match(text, expect)==0\n        ){\n          item->state = 1;\n        }else{\n          item->state = 7;\n        }\n        break;\n      };\n\n      case 1: {\n        int done = 0;\n        if( done ){\n          item->state = 7;\n        }\n        break;\n      }\n\n      default: {\n        if( ready(item) ){\n          if( (item->state & 2) ){\n            set_flag(item, 1);\n          }\n          item->state = 7;\n        }\n        break;\n      }\n    }\n  }\n\n  return rc;\n}\n",
        &[],
        "static int previous(int c) {\n    if( c==0 ) {\n        call(out, \"text %d\\n\", c);\n    } else\n\n        if( c==99 ) {\n            call(out, \"text %d\\n\", c);\n        } else\n\n        {\n            return 1;\n        }\n    return 0;\n}\n\nstatic int update(Item *item, const char *text) {\n    int rc = OK;\n\n    if( item->state<7 ) {\n        switch( item->state ) {\n        case 0: {\n            const char *expect = \"alpha\";\n            assert( length(expect)==5 );\n            if( item->safe==0\n                    && length(text)>=5\n                    && match(text, expect)==0\n              ) {\n                item->state = 1;\n            } else {\n                item->state = 7;\n            }\n            break;\n        };\n\n        case 1: {\n            int done = 0;\n            if( done ) {\n                item->state = 7;\n            }\n            break;\n        }\n\n        default: {\n            if( ready(item) ) {\n                if( (item->state & 2) ) {\n                    set_flag(item, 1);\n                }\n                item->state = 7;\n            }\n            break;\n        }\n        }\n    }\n\n    return rc;\n}\n",
    );
}

#[test]
fn string_argument_after_split_else_case_call_aligns_to_call_open_paren() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(t){\n    switch(value){\n      case ONE: {\n        if(ok){\n          call(stderr,\n               \"text\\n\", value);\n        }\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n        if(t) {\n            switch(value) {\n            case ONE: {\n                if(ok) {\n                    call(stderr,\n                         \"text\\n\", value);\n                }\n                break;\n            }\n            }\n        }\n}\n",
    );
}

#[test]
fn break_after_nested_switch_in_case_block_keeps_case_body_indent() {
    check(
        "int f(int value){\n  switch(value){\n    case 1: {\n      switch(value){\n        default:\n          goto target;\n      }\n      break;\n    }\n    default:\ntarget: {\n      return 0;\n    }\n  }\n}\n",
        &[],
        "int f(int value) {\n    switch(value) {\n    case 1: {\n        switch(value) {\n        default:\n            goto target;\n        }\n        break;\n    }\n    default:\ntarget: {\n            return 0;\n        }\n    }\n}\n",
    );
}

#[test]
fn break_after_expanded_nested_switch_labels_keeps_case_body_indent() {
    check(
        "const char *f(State *st, const char *s, const char *p) {\nagain:\n  if (p != st->end) {\n    switch (*p) {\n      case '(': {\n        if (*(p + 1) == ')')\n          s = first(st, s, p + 2);\n        else\n          s = second(st, s, p + 1);\n        break;\n      }\n      case ESC: {\n        switch (*(p + 1)) {\n          case 'b': {\n            s = call(st, s, p + 2);\n            if (s != NULL) {\n              p += 4; goto again;\n            }\n            break;\n          }\n          case '0': case '1': case '2': case '3':\n          case '4': case '5': case '6': case '7':\n          case '8': case '9': {\n            s = other(st, s, *(p + 1));\n            if (s != NULL) {\n              p += 2; goto again;\n            }\n            break;\n          }\n          default: goto fallback;\n        }\n        break;\n      }\n      default: fallback: {\n        const char *end = find(st, p);\n        return end;\n      }\n    }\n  }\n  return s;\n}\n",
        &[],
        "const char *f(State *st, const char *s, const char *p) {\nagain:\n    if (p != st->end) {\n        switch (*p) {\n        case '(': {\n            if (*(p + 1) == ')')\n                s = first(st, s, p + 2);\n            else\n                s = second(st, s, p + 1);\n            break;\n        }\n        case ESC: {\n            switch (*(p + 1)) {\n            case 'b': {\n                s = call(st, s, p + 2);\n                if (s != NULL) {\n                    p += 4;\n                    goto again;\n                }\n                break;\n            }\n            case '0':\n            case '1':\n            case '2':\n            case '3':\n            case '4':\n            case '5':\n            case '6':\n            case '7':\n            case '8':\n            case '9': {\n                s = other(st, s, *(p + 1));\n                if (s != NULL) {\n                    p += 2;\n                    goto again;\n                }\n                break;\n            }\n            default:\n                goto fallback;\n            }\n            break;\n        }\n        default:\nfallback: {\n                const char *end = find(st, p);\n                return end;\n            }\n        }\n    }\n    return s;\n}\n",
    );
}

#[test]
fn repeated_leading_block_comments_do_not_shift_case_brace_body() {
    check(
        "/*\n** Inspect the next item and record its width.\n*/\n/* placeholder record used to measure alignment */\n/* fallback */\n/*\n** Inspect the next item and record its layout details.\n** 'width' receives the item width, and 'alignment' receives its\n** required alignment.\n** Local variable 'offset' stores the value to align. The ALIGNED kind\n** always uses full alignment, while other kinds are limited by\n** the maximum alignment. The BYTE kind needs no alignment\n** regardless of its width.\n*/\n\nvoid f(int kind) {\n  while (next()) {\n    cursor++;\n    switch (kind) {\n      case INTEGER: {  /* integer values */\n        int result = call(cursor);\n        if (width < limit) {\n          int value = 1;\n        }\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "/*\n** Inspect the next item and record its width.\n*/\n/* placeholder record used to measure alignment */\n/* fallback */\n/*\n** Inspect the next item and record its layout details.\n** 'width' receives the item width, and 'alignment' receives its\n** required alignment.\n** Local variable 'offset' stores the value to align. The ALIGNED kind\n** always uses full alignment, while other kinds are limited by\n** the maximum alignment. The BYTE kind needs no alignment\n** regardless of its width.\n*/\n\nvoid f(int kind) {\n    while (next()) {\n        cursor++;\n        switch (kind) {\n        case INTEGER: {  /* integer values */\n            int result = call(cursor);\n            if (width < limit) {\n                int value = 1;\n            }\n            break;\n        }\n        }\n    }\n}\n",
    );
}

#[test]
fn block_comment_in_nested_split_else_if_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_FEATURE\n  if( c=='c' && n==2 ){\n    done();\n  }else\n#endif\n\n  if( c=='c' && n>=3 ){\n    if( argument_count==2 ){\n      set();\n    }else{\n      error();\n      rc = 1;\n    }\n  }else\n\n  /* comment */\n  if( c=='c' && n>=3 ){\n    rc = check();\n  }else\n\n#ifndef OMIT_FEATURE\n  if( c=='c' && clone(n) ){\n    if( argument_count==2 ){\n      try();\n    }else{\n      error();\n      rc = 1;\n    }\n  }else\n#endif\n\n  if( c=='c' && connection(n) ){\n    if( argument_count==1 ){\n      int i;\n      for(i=0; i<n; i++){\n        if( a ){\n          one();\n        }else if( b ){\n          two();\n        }\n      }\n    }else if( argument_count==2 ){\n      int i = value();\n      if( ok ){\n        use();\n      }\n    }else if( argument_count==3\n           && ready() ){\n      int i = value();\n      if( i<0 || i>=n ){\n        /* No-op */\n      }else if( active ){\n        error();\n        rc = 1;\n      }else if( handle ){\n        close();\n      }\n    }else{\n      usage();\n      rc = 1;\n    }\n  }else\n\n  if( c=='d' && n==4\n   && (starts_with(arg, \"left\", n)==0\n       || starts_with(arg,\"right\",n)==0)\n  ){\n    if( argument_count==2 ){\n#ifdef PLATFORM\n      if( flag ){\n        set();\n      }else{\n        clear();\n      }\n#else\n      clear();\n#endif\n    }\n    print();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_FEATURE\n    if( c=='c' && n==2 ) {\n        done();\n    } else\n#endif\n\n        if( c=='c' && n>=3 ) {\n            if( argument_count==2 ) {\n                set();\n            } else {\n                error();\n                rc = 1;\n            }\n        } else\n\n            /* comment */\n            if( c=='c' && n>=3 ) {\n                rc = check();\n            } else\n\n#ifndef OMIT_FEATURE\n                if( c=='c' && clone(n) ) {\n                    if( argument_count==2 ) {\n                        try();\n                    } else {\n                        error();\n                        rc = 1;\n                    }\n                } else\n#endif\n\n                    if( c=='c' && connection(n) ) {\n                        if( argument_count==1 ) {\n                            int i;\n                            for(i=0; i<n; i++) {\n                                if( a ) {\n                                    one();\n                                } else if( b ) {\n                                    two();\n                                }\n                            }\n                        } else if( argument_count==2 ) {\n                            int i = value();\n                            if( ok ) {\n                                use();\n                            }\n                        } else if( argument_count==3\n                                   && ready() ) {\n                            int i = value();\n                            if( i<0 || i>=n ) {\n                                /* No-op */\n                            } else if( active ) {\n                                error();\n                                rc = 1;\n                            } else if( handle ) {\n                                close();\n                            }\n                        } else {\n                            usage();\n                            rc = 1;\n                        }\n                    } else\n\n                        if( c=='d' && n==4\n                                && (starts_with(arg, \"left\", n)==0\n                                    || starts_with(arg,\"right\",n)==0)\n                          ) {\n                            if( argument_count==2 ) {\n#ifdef PLATFORM\n                                if( flag ) {\n                                    set();\n                                } else {\n                                    clear();\n                                }\n#else\n                                clear();\n#endif\n                            }\n                            print();\n                        }\n}\n",
    );
}

#[test]
fn multiline_call_before_split_else_keeps_else_at_header_indent() {
    check(
        "void f(void){\n#ifndef OMIT_FEATURE\n  if( a ){\n    first();\n  }else\n#endif\n\n  /* comment */\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    if( n ){\n#ifdef PLATFORM\n      set();\n#else\n      clear();\n#endif\n    }\n    print(\"x\",\n       value);\n  }else\n\n  if( d ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_FEATURE\n    if( a ) {\n        first();\n    } else\n#endif\n\n        /* comment */\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                if( n ) {\n#ifdef PLATFORM\n                    set();\n#else\n                    clear();\n#endif\n                }\n                print(\"x\",\n                      value);\n            } else\n\n                if( d ) {\n                    next();\n                }\n}\n",
    );
}

#[test]
fn preprocessor_split_else_after_multiline_calls_keeps_else_indent() {
    check(
        "void f(void){\n#ifndef OMIT_FIRST\n  if( a ){\n    first();\n  }else\n#endif\n\n#if defined(ENABLE_SECOND) \\\n  && !defined(OMIT_THIRD)\n  if( archive ){\n    second();\n  }else\n#endif\n\n#ifndef OMIT_THIRD\n  if( backup\n   || save\n  ){\n    rc = open(dest,\n              flags);\n    if( rc!=OK ){\n      error();\n      close(dest);\n      return 1;\n    }\n    if( async ){\n      exec(dest, \"pragma\",\n           0, 0, 0);\n    }\n    work();\n    if( rc==DONE ){\n      rc = 0;\n    }else{\n      error();\n      rc = 1;\n    }\n    close(dest);\n  }else\n#endif\n\n  if( bail ){\n    set();\n  }else\n\n  if( binary ){\n    old();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_FIRST\n    if( a ) {\n        first();\n    } else\n#endif\n\n#if defined(ENABLE_SECOND) \\\n  && !defined(OMIT_THIRD)\n        if( archive ) {\n            second();\n        } else\n#endif\n\n#ifndef OMIT_THIRD\n            if( backup\n                    || save\n              ) {\n                rc = open(dest,\n                          flags);\n                if( rc!=OK ) {\n                    error();\n                    close(dest);\n                    return 1;\n                }\n                if( async ) {\n                    exec(dest, \"pragma\",\n                         0, 0, 0);\n                }\n                work();\n                if( rc==DONE ) {\n                    rc = 0;\n                } else {\n                    error();\n                    rc = 1;\n                }\n                close(dest);\n            } else\n#endif\n\n                if( bail ) {\n                    set();\n                } else\n\n                    if( binary ) {\n                        old();\n                    }\n}\n",
    );
}

#[test]
fn preprocessor_branches_inside_split_else_branch_keep_nested_block_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if( a ){ first(); }else\n#endif\n\n  if( n ){\n    if( arg ){\n      set();\n    }else{\n      usage();\n      rc = 1;\n    }\n  }else\n\n  if( open ){\n    int mode = 0;\n    if( safe ) flags = READ;\n\n    for(i=1; i<count; i++){\n      const char *item = args[i];\n#ifndef OMIT_OPTION\n      if( option(item) ){\n        mode = 1;\n      }else\n#endif\n      if( item[0]=='-' ){\n        error();\n        rc = 1;\n        goto done;\n      }else if( name ){\n        extra();\n        rc = 1;\n        goto done;\n      }else{\n        name = item;\n      }\n    }\n\n    close_all();\n    handle = 0;\n\n    if( name || mode==HEX ){\n      if( fresh && name && !safe ){\n        if( prefix(name) ){\n          char *removed = uri(name);\n          check(removed);\n          delete(removed);\n          free(removed);\n        }else{\n          delete(name);\n        }\n      }\n#ifndef OMIT_OPTION\n      if( safe\n       && mode!=HEX\n       && name\n       && compare(name,\":temporary:\")!=0\n      ){\n        fail();\n      }\n#else\n      /* comment */\n#endif\n      if( name ){\n        new_name = copy(name);\n        check(new_name);\n      }else{\n        new_name = 0;\n      }\n      handle_name = new_name;\n      open_handle();\n      if( handle==0 ){\n        print();\n        free(new_name);\n      }else{\n        keep = new_name;\n      }\n    }\n  }\ndone:\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( n ) {\n            if( arg ) {\n                set();\n            } else {\n                usage();\n                rc = 1;\n            }\n        } else\n\n            if( open ) {\n                int mode = 0;\n                if( safe ) flags = READ;\n\n                for(i=1; i<count; i++) {\n                    const char *item = args[i];\n#ifndef OMIT_OPTION\n                    if( option(item) ) {\n                        mode = 1;\n                    } else\n#endif\n                        if( item[0]=='-' ) {\n                            error();\n                            rc = 1;\n                            goto done;\n                        } else if( name ) {\n                            extra();\n                            rc = 1;\n                            goto done;\n                        } else {\n                            name = item;\n                        }\n                }\n\n                close_all();\n                handle = 0;\n\n                if( name || mode==HEX ) {\n                    if( fresh && name && !safe ) {\n                        if( prefix(name) ) {\n                            char *removed = uri(name);\n                            check(removed);\n                            delete(removed);\n                            free(removed);\n                        } else {\n                            delete(name);\n                        }\n                    }\n#ifndef OMIT_OPTION\n                    if( safe\n                            && mode!=HEX\n                            && name\n                            && compare(name,\":temporary:\")!=0\n                      ) {\n                        fail();\n                    }\n#else\n                    /* comment */\n#endif\n                    if( name ) {\n                        new_name = copy(name);\n                        check(new_name);\n                    } else {\n                        new_name = 0;\n                    }\n                    handle_name = new_name;\n                    open_handle();\n                    if( handle==0 ) {\n                        print();\n                        free(new_name);\n                    } else {\n                        keep = new_name;\n                    }\n                }\n            }\ndone:\n}\n",
    );
}

#[test]
fn preprocessor_else_body_in_split_else_branch_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if( a ){ first(); }else\n#endif\n\n  if( b ){\n#ifndef OMIT_CHECK\n    if( safe\n     && mode\n     && name\n    ){\n      fail();\n    }\n#else\n    /* comment */\n#endif\n    if( name ){\n      open();\n    }else{\n      close();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n#ifndef OMIT_CHECK\n            if( safe\n                    && mode\n                    && name\n              ) {\n                fail();\n            }\n#else\n            /* comment */\n#endif\n            if( name ) {\n                open();\n            } else {\n                close();\n            }\n        }\n}\n",
    );
}

#[test]
fn assignment_logical_continuation_in_deep_split_else_keeps_value_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n  if(b){x();}else\n\n  if(c){x();}else\n\n  if( schema ){\n    int is_schema = same(name, \"one\")==0\n          || same(name, \"two\")==0\n          || same(name, \"three\")==0;\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    } else\n\n        if(b) {\n            x();\n        } else\n\n            if(c) {\n                x();\n            } else\n\n                if( schema ) {\n                    int is_schema = same(name, \"one\")==0\n                                    || same(name, \"two\")==0\n                                    || same(name, \"three\")==0;\n                    call();\n                }\n}\n",
    );
}

#[test]
fn statement_after_assignment_logical_continuation_in_split_else_keeps_block_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n  if( schema ){\n    int is_schema = same(name, \"one\")==0\n                   || same(name, \"two\")==0;\n    if( is_schema ){\n      print(out,\n            \"text\",\n            name\n           );\n    }\n    int glob = find(name, '*') != 0 ||\n               find(name, '?') != 0;\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    } else\n#endif\n\n        if( schema ) {\n            int is_schema = same(name, \"one\")==0\n                            || same(name, \"two\")==0;\n            if( is_schema ) {\n                print(out,\n                      \"text\",\n                      name\n                     );\n            }\n            int glob = find(name, '*') != 0 ||\n                       find(name, '?') != 0;\n        }\n}\n",
    );
}

#[test]
fn conditional_preprocessor_block_after_split_else_keeps_chain_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n#ifndef OMIT\n  if(b){x();}else\n#endif\n\n#ifdef DEBUG\n  /* comment one\n  ** comment two */\n  if(c){\n    x();\n  }else\n#endif\n\n  if(d){\n    y();\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    } else\n\n#ifndef OMIT\n        if(b) {\n            x();\n        } else\n#endif\n\n#ifdef DEBUG\n            /* comment one\n            ** comment two */\n            if(c) {\n                x();\n            } else\n#endif\n\n                if(d) {\n                    y();\n                }\n}\n",
    );
}

#[test]
fn operator_continuation_in_conditional_preprocessor_split_else_keeps_chain_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n#ifndef OMIT\n  if(b){x();}else\n#endif\n\n#ifdef DEBUG\n  /* comment one\n  ** comment two */\n  if(c){\n    x();\n  }else\n#endif\n\n  if(d){\n    if( call(alpha,beta,gamma,delta,epsilon,zeta,eta,theta)\n      != OK ){\n      value = 0;\n    }else{\n      value = 1;\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    } else\n\n#ifndef OMIT\n        if(b) {\n            x();\n        } else\n#endif\n\n#ifdef DEBUG\n            /* comment one\n            ** comment two */\n            if(c) {\n                x();\n            } else\n#endif\n\n                if(d) {\n                    if( call(alpha,beta,gamma,delta,epsilon,zeta,eta,theta)\n                            != OK ) {\n                        value = 0;\n                    } else {\n                        value = 1;\n                    }\n                }\n}\n",
    );
}

#[test]
fn body_after_multiline_else_if_in_conditional_split_else_keeps_header_body_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n#ifndef OMIT\n  if(b){x();}else\n#endif\n\n#ifdef DEBUG\n  /* comment one\n  ** comment two */\n  if(c){\n    x();\n  }else\n#endif\n\n  if(d){\n    if( one ){\n      first();\n    }else if( same(z,\"alpha\")==0 || same(z,\"beta\")==0\n           || same(z,\"gamma\")==0 || same(z,\"delta\")==0\n         ){\n      value = get(z);\n    }else if( same(z,\"debug\")==0 ){\n      debug = 1;\n    }else{\n      fail();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    } else\n\n#ifndef OMIT\n        if(b) {\n            x();\n        } else\n#endif\n\n#ifdef DEBUG\n            /* comment one\n            ** comment two */\n            if(c) {\n                x();\n            } else\n#endif\n\n                if(d) {\n                    if( one ) {\n                        first();\n                    } else if( same(z,\"alpha\")==0 || same(z,\"beta\")==0\n                               || same(z,\"gamma\")==0 || same(z,\"delta\")==0\n                             ) {\n                        value = get(z);\n                    } else if( same(z,\"debug\")==0 ) {\n                        debug = 1;\n                    } else {\n                        fail();\n                    }\n                }\n}\n",
    );
}

#[test]
fn deep_conditional_split_else_keeps_closing_brace_indent_after_string_calls() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(sha){\n    while(row()){\n      if( same(name, \"one\")==0 ){\n        append(query,\"SELECT value FROM alpha\"\n                     \" ORDER BY name;\", 0);\n      }else if( same(name, \"two\")==0 ){\n        append(query,\"SELECT value FROM beta\"\n                     \" ORDER BY name;\", 0);\n      }\n      append(sql, sep, 0);\n      append(sql, query, '\\\'');\n      value = 0;\n      append(sql, \",\", 0);\n      append(sql, name, '\\\'');\n      sep = \"),(\";\n    }\n    finalize(stmt);\n    if( separate ){\n      text = format(\n          \"%s))\"\n          \" SELECT value\",\n          sql);\n    }else{\n      text = format(\n          \"%s))\"\n          \" SELECT other\",\n          sql);\n    }\n    done();\n  }\n}\n");
    let indent = " ".repeat(33 * 4);
    expected.push_str(&format!(
        concat!(
            "{indent}if(sha) {{\n",
            "{indent}    while(row()) {{\n",
            "{indent}        if( same(name, \"one\")==0 ) {{\n",
            "{indent}            append(query,\"SELECT value FROM alpha\"\n",
            "{indent}                   \" ORDER BY name;\", 0);\n",
            "{indent}        }} else if( same(name, \"two\")==0 ) {{\n",
            "{indent}            append(query,\"SELECT value FROM beta\"\n",
            "{indent}                   \" ORDER BY name;\", 0);\n",
            "{indent}        }}\n",
            "{indent}        append(sql, sep, 0);\n",
            "{indent}        append(sql, query, '\\'');\n",
            "{indent}        value = 0;\n",
            "{indent}        append(sql, \",\", 0);\n",
            "{indent}        append(sql, name, '\\'');\n",
            "{indent}        sep = \"),(\";\n",
            "{indent}    }}\n",
            "{indent}    finalize(stmt);\n",
            "{indent}    if( separate ) {{\n",
            "{indent}        text = format(\n",
            "{indent}                   \"%s))\"\n",
            "{indent}                   \" SELECT value\",\n",
            "{indent}                   sql);\n",
            "{indent}    }} else {{\n",
            "{indent}        text = format(\n",
            "{indent}                   \"%s))\"\n",
            "{indent}                   \" SELECT other\",\n",
            "{indent}                   sql);\n",
            "{indent}    }}\n",
            "{indent}    done();\n",
            "{indent}}}\n",
            "}}\n",
        ),
        indent = indent,
    ));

    check(&input, &[], &expected);
}

#[test]
fn deep_split_else_keeps_multiline_preprocessor_branch_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(outer){\n    done();\n#if FEATURE \\\n && EXTRA\n    if( name ){\n      call(name,\n        \"alpha\"\n        \"beta\");\n    }\n#endif\n  }\n}\n");
    let outer = " ".repeat(132);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let call_arg = " ".repeat(145);
    expected.push_str(&format!(
        "{outer}if(outer) {{\n{body}done();\n#if FEATURE \\\n && EXTRA\n{body}if( name ) {{\n{nested}call(name,\n{call_arg}\"alpha\"\n{call_arg}\"beta\");\n{body}}}\n#endif\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn deep_split_else_preprocessor_block_keeps_string_assignment_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(outer){\n    if( debug ){\n      show();\n    }else{\n      exec();\n    }\n#if FEATURE\n    {\n      int rc;\n      char *text =\n        \"alpha\\n\"\n        \"beta\";\n      text = make(\n        \"gamma\"\n        \"delta\", text);\n      use(text);\n    }\n#endif\n  }\n}\n");
    let outer = " ".repeat(132);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let string_value = " ".repeat(144);
    let call_string = " ".repeat(151);
    expected.push_str(&format!(
        "{outer}if(outer) {{\n{body}if( debug ) {{\n{nested}show();\n{body}}} else {{\n{nested}exec();\n{body}}}\n#if FEATURE\n{body}{{\n{nested}int rc;\n{nested}char *text =\n{string_value}\"alpha\\n\"\n{string_value}\"beta\";\n{nested}text = make(\n{call_string}\"gamma\"\n{call_string}\"delta\", text);\n{nested}use(text);\n{body}}}\n#endif\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn multiline_preprocessor_split_else_closing_else_uses_header_indent() {
    check(
        "void f(void){\n  if(a){\n    first();\n  }else\n\n#ifndef B\n  if( (b)\n   || c\n  ){\n    int x;\n    if(x){\n      one();\n    }\n    after();\n  }else\n#endif\n\n  if(d){\n    last();\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        first();\n    } else\n\n#ifndef B\n        if( (b)\n                || c\n          ) {\n            int x;\n            if(x) {\n                one();\n            }\n            after();\n        } else\n#endif\n\n            if(d) {\n                last();\n            }\n}\n",
    );
}

#[test]
fn commented_call_in_preprocessor_split_else_keeps_following_statement_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("#if FEATURE\n  if( run\n   && (same(arg, \"one\")==0\n       || same(arg,\"two\")==0)\n  ){\n    char *cmd;\n    int i, rc;\n    cmd = make(has_space(arg[1])?\"%s\":\"\\\"%s\\\"\", arg[1]);\n    for(i=2; i<count && cmd!=0; i++){\n      cmd = make(has_space(arg[i])?\"%z %s\":\"%z \\\"%s\\\"\",\n                 cmd, arg[i]);\n    }\n    /* before */\n    rc = cmd!=0 ? run(cmd) : 1;\n    /* after */\n    free(cmd);\n    if( rc ) print(err,\"call failed: %d\\n\", rc);\n  }else\n#endif\n\n  if(next){ done(); }\n}\n");
    let header = " ".repeat(132);
    let header_condition = " ".repeat(140);
    let header_condition_tail = " ".repeat(144);
    let header_close = " ".repeat(134);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let continuation = " ".repeat(151);
    expected.push_str(&format!(
        "#if FEATURE\n{header}if( run\n{header_condition}&& (same(arg, \"one\")==0\n{header_condition_tail}|| same(arg,\"two\")==0)\n{header_close}) {{\n{body}char *cmd;\n{body}int i, rc;\n{body}cmd = make(has_space(arg[1])?\"%s\":\"\\\"%s\\\"\", arg[1]);\n{body}for(i=2; i<count && cmd!=0; i++) {{\n{nested}cmd = make(has_space(arg[i])?\"%z %s\":\"%z \\\"%s\\\"\",\n{continuation}cmd, arg[i]);\n{body}}}\n{body}/* before */\n{body}rc = cmd!=0 ? run(cmd) : 1;\n{body}/* after */\n{body}free(cmd);\n{body}if( rc ) print(err,\"call failed: %d\\n\", rc);\n{header}}} else\n#endif\n\n{body}if(next) {{\n{nested}done();\n{body}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn call_arguments_after_preprocessor_split_else_keep_call_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("#if FEATURE\n  if( run\n   && (same(arg, \"one\")==0\n       || same(arg,\"two\")==0)\n  ){\n    char *cmd;\n    cmd = make(\"%s\", arg);\n    /* before */\n    rc = cmd!=0 ? run(cmd) : 1;\n    /* after */\n    free(cmd);\n  }else\n#endif\n\n  if(show){\n    if( style==A\n     || style==B\n     || style==C\n    ){\n      call(out,\n        \"format %s %s %s\", \"mode\",\n        name, width,\n        flag==YES ? \"on\" : \"off\",\n        text==SQL ? \"\" : \"no\");\n    }else{\n      other();\n    }\n  }\n}\n");
    let header = " ".repeat(132);
    let header_condition = " ".repeat(140);
    let header_condition_tail = " ".repeat(144);
    let header_close = " ".repeat(134);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let condition = " ".repeat(148);
    let condition_close = " ".repeat(142);
    let call = " ".repeat(144);
    let argument = " ".repeat(149);
    expected.push_str(&format!(
        "#if FEATURE\n{header}if( run\n{header_condition}&& (same(arg, \"one\")==0\n{header_condition_tail}|| same(arg,\"two\")==0)\n{header_close}) {{\n{body}char *cmd;\n{body}cmd = make(\"%s\", arg);\n{body}/* before */\n{body}rc = cmd!=0 ? run(cmd) : 1;\n{body}/* after */\n{body}free(cmd);\n{header}}} else\n#endif\n\n{body}if(show) {{\n{nested}if( style==A\n{condition}|| style==B\n{condition}|| style==C\n{condition_close}) {{\n{call}call(out,\n{argument}\"format %s %s %s\", \"mode\",\n{argument}name, width,\n{argument}flag==YES ? \"on\" : \"off\",\n{argument}text==SQL ? \"\" : \"no\");\n{nested}}} else {{\n{call}other();\n{nested}}}\n{body}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn switch_cases_after_preprocessor_split_else_keep_switch_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("#if FEATURE\n  if(run){\n    run();\n  }else\n#endif\n\n  if(show){\n    switch(state){\n      case 0:  z = \"off\"; break;\n      default: z = \"on\"; break;\n      case 2:  z = \"two\"; break;\n    }\n  }\n}\n");
    let header = " ".repeat(132);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let switch_body = " ".repeat(144);
    expected.push_str(&format!(
        "#if FEATURE\n{header}if(run) {{\n{body}run();\n{header}}} else\n#endif\n\n{body}if(show) {{\n{nested}switch(state) {{\n{nested}case 0:\n{switch_body}z = \"off\";\n{switch_body}break;\n{nested}default:\n{switch_body}z = \"on\";\n{switch_body}break;\n{nested}case 2:\n{switch_body}z = \"two\";\n{switch_body}break;\n{nested}}}\n{body}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn deep_split_else_preprocessor_block_keeps_comment_string_call_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(outer){\n#if FEATURE\n    {\n      int rc;\n      char *text = \"alpha\";\n      text = make(\n        /* lower-case query runs first. */\n        \"with item as materialized(\\n\"\n        \"select name\\n\"\n        \"from value)\"\n        , text);\n      check(text);\n      if( debug ) print(out, \"%s\\n\", text);\n    }\n#endif\n  }\n}\n");
    let outer = " ".repeat(132);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let string_value = " ".repeat(144);
    expected.push_str(&format!(
        "{outer}if(outer) {{\n#if FEATURE\n{body}{{\n{nested}int rc;\n{nested}char *text = \"alpha\";\n{nested}text = make(\n{string_value}/* lower-case query runs first. */\n{string_value}\"with item as materialized(\\n\"\n{string_value}\"select name\\n\"\n{string_value}\"from value)\"\n{string_value}, text);\n{nested}check(text);\n{nested}if( debug ) print(out, \"%s\\n\", text);\n{body}}}\n#endif\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn logical_assignment_in_split_else_branch_keeps_following_statement_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(outer){\n#if FEATURE \\\n && EXTRA\n    if( name ){\n      int bGlob;\n      bGlob = find(name, '*') != 0 || find(name, '?') != 0 ||\n              find(name, '[') != 0;\n      if( dot(name) ){\n        first();\n      }else{\n        second();\n      }\n    }\n#endif\n  }\n}\n");
    let outer = " ".repeat(132);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let inner = " ".repeat(144);
    let continuation = " ".repeat(148);
    expected.push_str(&format!(
        "{outer}if(outer) {{\n#if FEATURE \\\n && EXTRA\n{body}if( name ) {{\n{nested}int bGlob;\n{nested}bGlob = find(name, '*') != 0 || find(name, '?') != 0 ||\n{continuation}find(name, '[') != 0;\n{nested}if( dot(name) ) {{\n{inner}first();\n{nested}}} else {{\n{inner}second();\n{nested}}}\n{body}}}\n#endif\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn deep_split_else_keeps_adjacent_string_call_indent_after_condition_assignment() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(outer){\n    if( name!=0 ){\n      int isSchema = same(name, \"one\")==0\n                  || same(name, \"two\")==0;\n      if( isSchema ){\n        print(out,\n          \"CREATE TABLE value (\\n\"\n          \"  name text\\n\"\n          \");\\n\",\n          same(\"temp\",name)==0 ? \"temp.\" : \"\"\n        );\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat(132);
    let body = " ".repeat(136);
    let nested = " ".repeat(140);
    let call = " ".repeat(144);
    let call_arg = " ".repeat(150);
    let assignment_tail = " ".repeat(155);
    let call_close = " ".repeat(149);
    expected.push_str(&format!(
        "{outer}if(outer) {{\n{body}if( name!=0 ) {{\n{nested}int isSchema = same(name, \"one\")==0\n{assignment_tail}|| same(name, \"two\")==0;\n{nested}if( isSchema ) {{\n{call}print(out,\n{call_arg}\"CREATE TABLE value (\\n\"\n{call_arg}\"  name text\\n\"\n{call_arg}\");\\n\",\n{call_arg}same(\"temp\",name)==0 ? \"temp.\" : \"\"\n{call_close});\n{nested}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn deep_conditional_split_else_keeps_preprocessor_block_brace_indent() {
    let mut input = String::from("void f(void){\n");
    let mut expected = String::from("void f(void) {\n");

    input.push_str("#ifndef OMIT\n  if(b0){\n    x0();\n  }else\n#endif\n\n");
    expected.push_str("#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n");

    for index in 1..32 {
        input.push_str(&format!("  if(b{index}){{\n    x{index}();\n  }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(sha){\n    done();\n#if FEATURE\n    {\n      int value;\n      value = 1;\n    }\n#endif\n  }\n}\n");
    let outer = " ".repeat(33 * 4);
    let body = " ".repeat(34 * 4);
    let nested = " ".repeat(35 * 4);
    expected.push_str(&format!(
        "{outer}if(sha) {{\n{body}done();\n#if FEATURE\n{body}{{\n{nested}int value;\n{nested}value = 1;\n{body}}}\n#endif\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn else_if_after_guarded_braceless_else_keeps_enclosing_block_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n  if( prompt ){\n    for(i=0; i<n; i++){\n      if( option ){\n        set();\n      }else\n#ifndef COLOR\n      if( color ){\n        on();\n      }else\n#endif\n      if( dash ){\n        no();\n      }else{\n        bad();\n      }\n    }else if( extra ){\n      fail();\n    }else{\n      save();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    } else\n#endif\n\n        if( prompt ) {\n            for(i=0; i<n; i++) {\n                if( option ) {\n                    set();\n                } else\n#ifndef COLOR\n                    if( color ) {\n                        on();\n                    } else\n#endif\n                        if( dash ) {\n                            no();\n                        } else {\n                            bad();\n                        }\n            } else if( extra ) {\n                fail();\n            } else {\n                save();\n            }\n        }\n}\n",
    );
}

#[test]
fn continuation_after_split_else_endif_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n#ifndef OMIT_BRANCH\n  if( r ){\n    call();\n  }else\n#endif\n\n  if( c=='s' &&\n      (same(arg, \"one\")==0 ||\n       same(arg, \"two\")==0)\n    ){\n    open(\n      arg, value\n    );\n  }else\n\n  if( next ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    } else\n#endif\n\n#ifndef OMIT_BRANCH\n        if( r ) {\n            call();\n        } else\n#endif\n\n            if( c=='s' &&\n                    (same(arg, \"one\")==0 ||\n                     same(arg, \"two\")==0)\n              ) {\n                open(\n                    arg, value\n                );\n            } else\n\n                if( next ) {\n                    next();\n                }\n}\n",
    );
}

#[test]
fn endif_before_else_if_in_split_else_body_keeps_enclosing_if_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n  if( read ){\n    if( pipe ){\n#ifdef OMIT\n      error();\n#else\n      open();\n      if( ok ){\n        read_pipe();\n      }else{\n        fail();\n      }\n#endif\n    }else if( file ){\n      read_file();\n    }else{\n      none();\n    }\n    done();\n  }else\n\n  if( next ){\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    } else\n#endif\n\n        if( read ) {\n            if( pipe ) {\n#ifdef OMIT\n                error();\n#else\n                open();\n                if( ok ) {\n                    read_pipe();\n                } else {\n                    fail();\n                }\n#endif\n            } else if( file ) {\n                read_file();\n            } else {\n                none();\n            }\n            done();\n        } else\n\n            if( next ) {\n                call();\n            }\n}\n",
    );
}

#[test]
fn nested_condition_operator_in_split_else_aligns_to_inner_paren() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n  if( c=='i' && (same(arg, \"one\")==0\n       || same(arg, \"two\")==0)\n  ){\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    } else\n#endif\n\n        if( c=='i' && (same(arg, \"one\")==0\n                       || same(arg, \"two\")==0)\n          ) {\n            call();\n        }\n}\n",
    );
}

#[test]
fn adjacent_string_call_in_split_else_keeps_following_block_indent() {
    check(
        "void f(void){\n  if( a ){\n    first();\n  }else\n\n  if( b ){\n    for(i=0; i<n; i++){\n      if( match(i) ){\n        if( value<0 ){\n          report(error,\"Error: %s\\n\"\n                \"Use help\\n\", arg);\n          rc = 1;\n          goto done;\n        }\n      }\n    }\n  }\ndone:\n}\n",
        &[],
        "void f(void) {\n    if( a ) {\n        first();\n    } else\n\n        if( b ) {\n            for(i=0; i<n; i++) {\n                if( match(i) ) {\n                    if( value<0 ) {\n                        report(error,\"Error: %s\\n\"\n                               \"Use help\\n\", arg);\n                        rc = 1;\n                        goto done;\n                    }\n                }\n            }\n        }\ndone:\n}\n",
    );
}

#[test]
fn switch_case_after_preprocessor_split_else_keeps_nested_if_body_indent() {
    check(
        r#"void f(int value){
  if(a){
    first();
  }else

#if FEATURE
  if(b){
    second();
  }else
#endif

  if(c){
    switch(value){
      case ONE: {
        if(ready){
          call();
        }
        break;
      }
    }
  }
}
"#,
        &[],
        r#"void f(int value) {
    if(a) {
        first();
    } else

#if FEATURE
        if(b) {
            second();
        } else
#endif

            if(c) {
                switch(value) {
                case ONE: {
                    if(ready) {
                        call();
                    }
                    break;
                }
                }
            }
}
"#,
    );
}

#[test]
fn else_if_body_after_switch_in_split_else_keeps_branch_indent() {
    check(
        r#"void f(int value){
#if FEATURE
  if( a0 ){ call0(); }else
#endif

  if( a1 ){ call1(); }else

  if( a2 ){ call2(); }else

#if FEATURE
  if( a3 ){ call3(); }else
#endif

  if( b ){
    int ok = 0;
    switch(value){
      case ONE: {
        ok = 1;
        break;
      }
      case TWO: {
        int x;
        if( n>=3 ){
          x = value(arg);
          call(db, schema, code, &x);
        }
        ok = 2;
        break;
      }
    }
    if( ok==0 && index>=0 ){
      report(out, "Usage: %s %s\\n",
              name, items[index].usage);
      rc = 1;
    }else if( ok==1 ){
      char text[100];
      write(text, "%lld", result);
      report(out, "%s\\n", text);
    }
  }else

  if( c ){
    next();
  }
}
"#,
        &[],
        r#"void f(int value) {
#if FEATURE
    if( a0 ) {
        call0();
    } else
#endif

        if( a1 ) {
            call1();
        } else

            if( a2 ) {
                call2();
            } else

#if FEATURE
                if( a3 ) {
                    call3();
                } else
#endif

                    if( b ) {
                        int ok = 0;
                        switch(value) {
                        case ONE: {
                            ok = 1;
                            break;
                        }
                        case TWO: {
                            int x;
                            if( n>=3 ) {
                                x = value(arg);
                                call(db, schema, code, &x);
                            }
                            ok = 2;
                            break;
                        }
                        }
                        if( ok==0 && index>=0 ) {
                            report(out, "Usage: %s %s\\n",
                                   name, items[index].usage);
                            rc = 1;
                        } else if( ok==1 ) {
                            char text[100];
                            write(text, "%lld", result);
                            report(out, "%s\\n", text);
                        }
                    } else

                        if( c ) {
                            next();
                        }
}
"#,
    );
}

#[test]
fn statement_after_switch_in_split_else_keeps_branch_indent() {
    check(
        "void f(int value){\n  if( a ){\n    first();\n  }else\n\n  if( b ){\n    int ok = 0;\n    switch(value){\n      case 1: {\n        if( value ){\n          call();\n        }\n        ok = 1;\n        break;\n      }\n    }\n    if( ok ){\n      done();\n    }\n  }else\n\n  if( c ){\n    next();\n  }\n}\n",
        &[],
        "void f(int value) {\n    if( a ) {\n        first();\n    } else\n\n        if( b ) {\n            int ok = 0;\n            switch(value) {\n            case 1: {\n                if( value ) {\n                    call();\n                }\n                ok = 1;\n                break;\n            }\n            }\n            if( ok ) {\n                done();\n            }\n        } else\n\n            if( c ) {\n                next();\n            }\n}\n",
    );
}

#[test]
fn nested_case_if_in_split_else_keeps_following_statement_indent() {
    check(
        "void f(int value){\n  if( a ){\n    first();\n  }else\n\n  if( b ){\n    switch(value){\n      case 1: {\n        char *text = get();\n        if( text ){\n          print(\"%s\\n\", text);\n          free(text);\n        }\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n    if( a ) {\n        first();\n    } else\n\n        if( b ) {\n            switch(value) {\n            case 1: {\n                char *text = get();\n                if( text ) {\n                    print(\"%s\\n\", text);\n                    free(text);\n                }\n                break;\n            }\n            }\n        }\n}\n",
    );
}

#[test]
fn adjacent_string_call_before_else_in_split_else_keeps_else_indent() {
    check(
        "void f(void){\n  if( a ){\n    first();\n  }else\n\n  if( b ){\n    if( value<0 ){\n      report(error,\"Error: %s\\n\"\n            \"Use help\\n\", arg);\n    }else{\n      use();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n    if( a ) {\n        first();\n    } else\n\n        if( b ) {\n            if( value<0 ) {\n                report(error,\"Error: %s\\n\"\n                       \"Use help\\n\", arg);\n            } else {\n                use();\n            }\n        }\n}\n",
    );
}

#[test]
fn nested_split_else_condition_and_call_continuation_keep_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  /* comment */\n  if( b ){\n    second();\n  }else\n\n  if( c=='c' && n==4\n   && (match_token(items[0], \"west\", n)==0\n       || match_token(items[0],\"east\",n)==0)\n  ){\n    if( size==2 ){\n#ifdef SYS\n      set();\n#else\n      clear();\n#endif\n    }\n    print_line(output, \"mode is %s\\n\",\n               (state->flags & MODE_FLAG)!=0 ? \"ON\" : \"OFF\");\n  }else\n\n  if( d ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        /* comment */\n        if( b ) {\n            second();\n        } else\n\n            if( c=='c' && n==4\n                    && (match_token(items[0], \"west\", n)==0\n                        || match_token(items[0],\"east\",n)==0)\n              ) {\n                if( size==2 ) {\n#ifdef SYS\n                    set();\n#else\n                    clear();\n#endif\n                }\n                print_line(output, \"mode is %s\\n\",\n                           (state->flags & MODE_FLAG)!=0 ? \"ON\" : \"OFF\");\n            } else\n\n                if( d ) {\n                    next();\n                }\n}\n",
    );
}

#[test]
fn multiline_ternary_call_in_split_else_keeps_call_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  /* comment */\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    for(i=0; i<n; i++){\n      print_line(out, \"%s: %s %s%s\\n\",\n                 name, file, readonly ? \"r/o\" : \"r/w\",\n                 state==NONE ? \"\" :\n                 state==READ ? \" read\" : \" write\");\n      free(name);\n      free(file);\n    }\n    clear(names);\n  }else\n\n  if( d ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        /* comment */\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                for(i=0; i<n; i++) {\n                    print_line(out, \"%s: %s %s%s\\n\",\n                               name, file, readonly ? \"r/o\" : \"r/w\",\n                               state==NONE ? \"\" :\n                               state==READ ? \" read\" : \" write\");\n                    free(name);\n                    free(file);\n                }\n                clear(names);\n            } else\n\n                if( d ) {\n                    next();\n                }\n}\n",
    );
}

#[test]
fn local_struct_in_split_else_branch_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  /* comment */\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    done();\n  }else\n\n  if( d ){\n    static const struct Choice {\n      const char *name;\n      int op;\n    } items[] = {\n      { \"one\", 1 },\n    };\n    int i;\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        /* comment */\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                done();\n            } else\n\n                if( d ) {\n                    static const struct Choice {\n                        const char *name;\n                        int op;\n                    } items[] = {\n                        { \"one\", 1 },\n                    };\n                    int i;\n                }\n}\n",
    );
}

#[test]
fn commented_initializer_rows_in_split_else_keep_row_indent() {
    check(
        "void f(void){\n#if FEATURE\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    static const struct {\n       const char *name;\n       int value;\n    } items[] = {\n      { \"alpha\", 1 },\n   /* { \"beta\", 2 },*/\n      { \"gamma\", 3 },\n   /* { \"delta\", 4 },*/\n    };\n  }\n}\n",
        &[],
        "void f(void) {\n#if FEATURE\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            static const struct {\n                const char *name;\n                int value;\n            } items[] = {\n                { \"alpha\", 1 },\n                /* { \"beta\", 2 },*/\n                { \"gamma\", 3 },\n                /* { \"delta\", 4 },*/\n            };\n        }\n}\n",
    );
}

#[test]
fn preprocessor_branch_initializer_rows_in_split_else_keep_row_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    static const struct {\n      const char *name;\n      int code;\n    } items[] = {\n      {\"one\",1},\n#ifdef FEATURE\n      {\"two\",2},\n#endif\n      {\"three\",3},\n    };\n    done();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n        if(b1) {\n            x1();\n        } else\n\n            if(t) {\n                static const struct {\n                    const char *name;\n                    int code;\n                } items[] = {\n                    {\"one\",1},\n#ifdef FEATURE\n                    {\"two\",2},\n#endif\n                    {\"three\",3},\n                };\n                done();\n            }\n}\n",
    );
}

#[test]
fn multiline_condition_in_split_else_keeps_branch_continuation_indent() {
    check(
        "void f(void){\n#if FEATURE\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    if( value[0]=='-'\n     && (same(value,\"--one\")==0 || same(value,\"-one\")==0)\n     && n>=4\n    ){\n      x = 1;\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#if FEATURE\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            if( value[0]=='-'\n                    && (same(value,\"--one\")==0 || same(value,\"-one\")==0)\n                    && n>=4\n              ) {\n                x = 1;\n            }\n        }\n}\n",
    );
}

#[test]
fn statement_after_multiline_condition_in_deep_split_else_keeps_branch_indent() {
    check(
        "void f(void){\n#if FEATURE\n  if( a0 ){ call0(); }else\n#endif\n\n  if( a1 ){ call1(); }else\n\n  if( a2 ){ call2(); }else\n\n#if FEATURE\n  if( a3 ){ call3(); }else\n#endif\n\n  if( b ){\n    if( value[0]=='-'\n     && (same(value,\"--one\")==0 || same(value,\"-one\")==0)\n     && n>=4\n    ){\n      x = 1;\n    }\n\n    /* comment */\n    if( same(cmd,\"help\")==0 ){\n      call();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#if FEATURE\n    if( a0 ) {\n        call0();\n    } else\n#endif\n\n        if( a1 ) {\n            call1();\n        } else\n\n            if( a2 ) {\n                call2();\n            } else\n\n#if FEATURE\n                if( a3 ) {\n                    call3();\n                } else\n#endif\n\n                    if( b ) {\n                        if( value[0]=='-'\n                                && (same(value,\"--one\")==0 || same(value,\"-one\")==0)\n                                && n>=4\n                          ) {\n                            x = 1;\n                        }\n\n                        /* comment */\n                        if( same(cmd,\"help\")==0 ) {\n                            call();\n                        }\n                    }\n}\n",
    );
}

#[test]
fn switch_cases_in_deep_split_else_keep_case_body_indent() {
    check(
        "void f(void){\n#if FEATURE\n  if( a0 ){ call0(); }else\n#endif\n\n  if( a1 ){ call1(); }else\n\n  if( a2 ){ call2(); }else\n\n#if FEATURE\n  if( a3 ){ call3(); }else\n#endif\n\n  if( b ){\n    if( bad ){\n      error();\n    }else{\n      switch(value){\n        case ONE: {\n          if( n!=2 ) break;\n          ok = 1;\n          break;\n        }\n        case TWO: {\n          int x;\n          if( n!=3 ) break;\n          x = value();\n          if( x ){\n            use(x);\n          }\n          ok = 2;\n          break;\n        }\n      }\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#if FEATURE\n    if( a0 ) {\n        call0();\n    } else\n#endif\n\n        if( a1 ) {\n            call1();\n        } else\n\n            if( a2 ) {\n                call2();\n            } else\n\n#if FEATURE\n                if( a3 ) {\n                    call3();\n                } else\n#endif\n\n                    if( b ) {\n                        if( bad ) {\n                            error();\n                        } else {\n                            switch(value) {\n                            case ONE: {\n                                if( n!=2 ) break;\n                                ok = 1;\n                                break;\n                            }\n                            case TWO: {\n                                int x;\n                                if( n!=3 ) break;\n                                x = value();\n                                if( x ) {\n                                    use(x);\n                                }\n                                ok = 2;\n                                break;\n                            }\n                            }\n                        }\n                    }\n}\n",
    );
}

#[test]
fn local_struct_in_switch_case_after_split_else_keeps_case_body_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n\n      /* comment */\n      case ONE: {\n        static const struct {\n          int id;\n          const char *name;\n        } items[] = {\n          {1, \"one\"},\n          {2, \"two\"},\n        };\n        done();\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n        if(b1) {\n            x1();\n        } else\n\n            if(t) {\n                switch(value) {\n\n                /* comment */\n                case ONE: {\n                    static const struct {\n                        int id;\n                        const char *name;\n                    } items[] = {\n                        {1, \"one\"},\n                        {2, \"two\"},\n                    };\n                    done();\n                    break;\n                }\n                }\n            }\n}\n",
    );
}

#[test]
fn statement_after_multiline_call_in_case_after_split_else_keeps_block_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n      case ONE: {\n        if( use ){\n          int jj;\n          if( jj>=count ){\n            print(err,\n                  \"Error: %s\\n\", text);\n            puts(\"one\", err);\n            for(jj=0; jj<count; jj++){\n              print(err, \" %s\", labels[jj]);\n            }\n            puts(\"\\n\", err);\n            rc = 1;\n            goto done;\n          }\n        }\n        break;\n      }\n    }\n  }\ndone:\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n        if(b1) {\n            x1();\n        } else\n\n            if(t) {\n                switch(value) {\n                case ONE: {\n                    if( use ) {\n                        int jj;\n                        if( jj>=count ) {\n                            print(err,\n                                  \"Error: %s\\n\", text);\n                            puts(\"one\", err);\n                            for(jj=0; jj<count; jj++) {\n                                print(err, \" %s\", labels[jj]);\n                            }\n                            puts(\"\\n\", err);\n                            rc = 1;\n                            goto done;\n                        }\n                    }\n                    break;\n                }\n                }\n            }\ndone:\n}\n",
    );
}

#[test]
fn commented_struct_members_in_switch_case_after_split_else_keep_case_body_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n      case ONE: {\n        static const struct {\n          unsigned int mask;    /* Mask */\n          unsigned int show;  /* Display */\n          const char *name;   /* Name */\n        } items[] = {\n          { 1, 1, \"one\" },\n        };\n        unsigned int cur;\n        unsigned int next;\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n        if(b1) {\n            x1();\n        } else\n\n            if(t) {\n                switch(value) {\n                case ONE: {\n                    static const struct {\n                        unsigned int mask;    /* Mask */\n                        unsigned int show;  /* Display */\n                        const char *name;   /* Name */\n                    } items[] = {\n                        { 1, 1, \"one\" },\n                    };\n                    unsigned int cur;\n                    unsigned int next;\n                    break;\n                }\n                }\n            }\n}\n",
    );
}

#[test]
fn long_initializer_in_switch_case_after_split_else_keeps_case_body_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n      case ONE: {\n        static const struct {\n          int id;\n        } items[] = {\n          {1},\n          {2},\n          {3},\n          {4},\n          {5},\n          {6},\n          {7},\n          {8},\n          {9},\n          {10},\n          {11},\n          {12},\n          {13},\n          {14},\n          {15},\n          {16},\n          {17},\n          {18},\n          {19},\n          {20},\n          {21},\n          {22},\n          {23},\n          {24},\n          {25},\n          {26},\n          {27},\n          {28},\n          {29},\n          {30},\n          {31},\n          {32},\n          {33},\n          {34},\n          {35},\n        };\n        for(i=0; i<n; i++){\n          if(i==0){\n            one();\n          }else if(i==1){\n            two();\n          }else{\n            three();\n          }\n        }\n        break;\n      }\n      /* next */\n      case TWO: {\n        if(flag){\n          call(arg,\n               value);\n          done();\n        }\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n        if(b1) {\n            x1();\n        } else\n\n            if(t) {\n                switch(value) {\n                case ONE: {\n                    static const struct {\n                        int id;\n                    } items[] = {\n                        {1},\n                        {2},\n                        {3},\n                        {4},\n                        {5},\n                        {6},\n                        {7},\n                        {8},\n                        {9},\n                        {10},\n                        {11},\n                        {12},\n                        {13},\n                        {14},\n                        {15},\n                        {16},\n                        {17},\n                        {18},\n                        {19},\n                        {20},\n                        {21},\n                        {22},\n                        {23},\n                        {24},\n                        {25},\n                        {26},\n                        {27},\n                        {28},\n                        {29},\n                        {30},\n                        {31},\n                        {32},\n                        {33},\n                        {34},\n                        {35},\n                    };\n                    for(i=0; i<n; i++) {\n                        if(i==0) {\n                            one();\n                        } else if(i==1) {\n                            two();\n                        } else {\n                            three();\n                        }\n                    }\n                    break;\n                }\n                /* next */\n                case TWO: {\n                    if(flag) {\n                        call(arg,\n                             value);\n                        done();\n                    }\n                    break;\n                }\n                }\n            }\n}\n",
    );
}

#[test]
fn long_preprocessor_split_else_chain_keeps_branch_indent() {
    let depth = 64;
    let mut input = String::from("void f(void){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(void) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  done();\n}\n");
    let indent = " ".repeat((depth + 1) * 4);
    expected.push_str(&format!("{indent}done();\n}}\n"));

    check(&input, &[], &expected);
}

#[test]
fn multiline_call_in_switch_case_after_long_split_else_keeps_call_indent() {
    let depth = 64;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n      case ONE:\n        call(one);\n        break;\n      case TWO:\n        if(flag){\n          result = call(alpha, beta,\n                        gamma,\n                        delta);\n          done = 1;\n        }\n        break;\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let call_indent = " ".repeat((depth + 4) * 4 + "result = call(".len());
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}switch(value) {{\n{body}case ONE:\n{nested}call(one);\n{nested}break;\n{body}case TWO:\n{nested}if(flag) {{\n{inside}result = call(alpha, beta,\n{call_indent}gamma,\n{call_indent}delta);\n{inside}done = 1;\n{nested}}}\n{nested}break;\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn long_split_else_preprocessor_case_keeps_post_while_statement_indent() {
    let depth = 64;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    expected.push_str(&format!("{outer}if(t) {{\n{body}switch(value) {{\n"));
    for index in 0..20 {
        input.push_str(&format!(
            "      case C{index}:\n        if(value){{\n          call();\n        }}\n        break;\n\n"
        ));
        expected.push_str(&format!(
            "{body}case C{index}:\n{nested}if(value) {{\n{inside}call();\n{nested}}}\n{nested}break;\n\n"
        ));
    }
    input.push_str("#ifdef DEBUG\n      case ONE: {\n        if(value==4){\n          one();\n        }else if(value==3){\n          two();\n        }else if(value==2){\n          int id = 1;\n          while(1){\n            int val = 0;\n            call(id, &val);\n            if( val==0 ) break;\n            if( id>1 ) print(\" \");\n            print(\"%d\", id);\n            id++;\n          }\n          if( id>1 ) print(\"\\n\");\n          done = 1;\n        }\n        break;\n      }\n#endif\n    }\n  }\n}\n");
    expected.push_str(&format!(
        "#ifdef DEBUG\n{body}case ONE: {{\n{nested}if(value==4) {{\n{inside}one();\n{nested}}} else if(value==3) {{\n{inside}two();\n{nested}}} else if(value==2) {{\n{inside}int id = 1;\n{inside}while(1) {{\n{inside}    int val = 0;\n{inside}    call(id, &val);\n{inside}    if( val==0 ) break;\n{inside}    if( id>1 ) print(\" \");\n{inside}    print(\"%d\", id);\n{inside}    id++;\n{inside}}}\n{inside}if( id>1 ) print(\"\\n\");\n{inside}done = 1;\n{nested}}}\n{nested}break;\n{body}}}\n#endif\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn long_split_else_case_keeps_statement_after_braceless_break_indent() {
    let depth = 8;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(b){\n    if(value<0){\n      print(out,\n            \"message\\n\");\n    }else{\n      switch(value){\n        case ONE: {\n          if( count!=2 && count!=3 ) break;\n          result = count==3 ? value(arg[2]) : -1;\n          call(result);\n          break;\n        }\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let string_indent = " ".repeat((depth + 3) * 4 + "print(".len());
    expected.push_str(&format!(
        "{outer}if(b) {{\n{body}if(value<0) {{\n{nested}print(out,\n{string_indent}\"message\\n\");\n{body}}} else {{\n{nested}switch(value) {{\n{nested}case ONE: {{\n{inside}if( count!=2 && count!=3 ) break;\n{inside}result = count==3 ? value(arg[2]) : -1;\n{inside}call(result);\n{inside}break;\n{nested}}}\n{nested}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn long_split_else_nested_case_call_keeps_following_statement_indent() {
    let depth = 8;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(b){\n    switch(value){\n      case ONE: {\n        if(flag){\n          for(jj=0; jj<count; jj++){\n            if(found()) break;\n          }\n          if(jj>=count){\n            print(err,\n                  \"Error: %s\\n\", label);\n            puts(\"next\", err);\n            for(jj=0; jj<count; jj++){\n              print(err, \" %s\", names[jj]);\n            }\n            rc = 1;\n            goto exit;\n          }\n        }\n        break;\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let deep = " ".repeat((depth + 5) * 4);
    let call_arg = " ".repeat((depth + 5) * 4 + "print(".len());
    expected.push_str(&format!(
        "{outer}if(b) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}if(flag) {{\n{inside}for(jj=0; jj<count; jj++) {{\n{deep}if(found()) break;\n{inside}}}\n{inside}if(jj>=count) {{\n{deep}print(err,\n{call_arg}\"Error: %s\\n\", label);\n{deep}puts(\"next\", err);\n{deep}for(jj=0; jj<count; jj++) {{\n{deep}    print(err, \" %s\", names[jj]);\n{deep}}}\n{deep}rc = 1;\n{deep}goto exit;\n{inside}}}\n{nested}}}\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn call_argument_after_comma_in_case_after_long_split_else_aligns_to_call_paren() {
    let depth = 8;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(b){\n    switch(value){\n      case ONE: {\n        if(status){\n          print(out, \"value: %d\\n\",\n                state.value);\n          print(out, \"next: %d\\n\",\n                state.next);\n        }\n        break;\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let call_arg = " ".repeat((depth + 4) * 4 + "print(".len());
    expected.push_str(&format!(
        "{outer}if(b) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}if(status) {{\n{inside}print(out, \"value: %d\\n\",\n{call_arg}state.value);\n{inside}print(out, \"next: %d\\n\",\n{call_arg}state.next);\n{nested}}}\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn argument_after_string_literal_in_case_call_aligns_to_string_argument() {
    let depth = 8;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(b){\n    switch(value){\n      case ONE: {\n        if(flag){\n          print(err,\n                \"message: %s\\n\",\n                arg);\n          rc = 1;\n        }\n        break;\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let call_arg = " ".repeat((depth + 4) * 4 + "print(".len());
    expected.push_str(&format!(
        "{outer}if(b) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}if(flag) {{\n{inside}print(err,\n{call_arg}\"message: %s\\n\",\n{call_arg}arg);\n{inside}rc = 1;\n{nested}}}\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn adjacent_string_call_in_case_after_long_split_else_keeps_string_indent() {
    let depth = 8;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(b){\n    switch(value){\n      case ONE: {\n        if(help){\n          puts(\n            \"Usage: command ARGS\\n\"\n            \"Possible arguments:\\n\"\n            \"   on\\n\"\n            \"   off\\n\"\n            ,out\n          );\n        }\n        break;\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let call_arg = " ".repeat((depth + 5) * 4);
    expected.push_str(&format!(
        "{outer}if(b) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}if(help) {{\n{inside}puts(\n{call_arg}\"Usage: command ARGS\\n\"\n{call_arg}\"Possible arguments:\\n\"\n{call_arg}\"   on\\n\"\n{call_arg}\"   off\\n\"\n{call_arg},out\n{inside});\n{nested}}}\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn adjacent_string_call_over_max_in_case_after_long_split_else_uses_block_continuation_indent() {
    let depth = 64;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n      case ONE: {\n        if(help){\n          message_write(\n            \"Usage: command ARGS\\n\"\n            \"Possible arguments:\\n\"\n            ,out\n          );\n        }\n        break;\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let call_arg = " ".repeat((depth + 5) * 4);
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}if(help) {{\n{inside}message_write(\n{call_arg}\"Usage: command ARGS\\n\"\n{call_arg}\"Possible arguments:\\n\"\n{call_arg},out\n{inside});\n{nested}}}\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn assigned_string_call_close_after_long_split_else_aligns_to_value_column() {
    let depth = 8;
    let mut input = String::from("void f(void){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(void) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    char *text = format(\n      \"alpha\"\n      \"beta\"\n      \"gamma\",\n      value, value\n    );\n    next();\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let inside = " ".repeat((depth + 2) * 4 + "char *text = ".len());
    let arg = " ".repeat((depth + 2) * 4 + "char *text = ".len() + 4);
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}char *text = format(\n{arg}\"alpha\"\n{arg}\"beta\"\n{arg}\"gamma\",\n{arg}value, value\n{inside});\n{body}next();\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn call_argument_after_line_comment_string_in_long_split_else_aligns_to_call_paren() {
    let depth = 8;
    let mut input =
        String::from("void f(int c){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(c){\n    print(out, \"Value %s %s\\n\" /*info*/,\n          value(), source());\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let arg = " ".repeat((depth + 2) * 4 + "print(".len());
    expected.push_str(&format!(
        "{outer}if(c) {{\n{body}print(out, \"Value %s %s\\n\" /*info*/,\n{arg}value(), source());\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn macro_string_call_argument_after_long_split_else_aligns_to_call_paren() {
    let depth = 8;
    let mut input =
        String::from("void f(int c){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(c){\n    print(out, \"prefix\" VALUE_ONE \".\"\n          VALUE_TWO \".\"\n          VALUE_THREE \" end\", value);\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let arg = " ".repeat((depth + 2) * 4 + "print(".len());
    expected.push_str(&format!(
        "{outer}if(c) {{\n{body}print(out, \"prefix\" VALUE_ONE \".\"\n{arg}VALUE_TWO \".\"\n{arg}VALUE_THREE \" end\", value);\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn condition_sibling_operator_after_nested_group_in_long_split_else_keeps_group_indent() {
    let depth = 8;
    let mut input =
        String::from("void f(int c){\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if( (c==1\n        && (same(arg, \"alpha\")\n            || same(arg, \"beta\"))\n        || (c==2 && same(arg,\"gamma\"))\n        || (c==3 && same(arg,\"delta\"))\n  ) {\n    done();\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let branch = " ".repeat((depth + 3) * 4);
    let inner = " ".repeat((depth + 4) * 4);
    expected.push_str(&format!(
        "{outer}if( (c==1\n{branch}&& (same(arg, \"alpha\")\n{inner}|| same(arg, \"beta\"))\n{branch}|| (c==2 && same(arg,\"gamma\"))\n{branch}|| (c==3 && same(arg,\"delta\"))\n{outer}) {{\n{outer}done();\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn call_close_after_string_in_case_after_long_split_else_aligns_to_call_paren() {
    let depth = 64;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n      case ONE: {\n        if(value){\n          call(stderr,\n            \"message\\n\"\n          );\n          rc = 1;\n        }\n        break;\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let string_indent = " ".repeat((depth + 4) * 4 + "call(".len());
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}if(value) {{\n{inside}call(stderr,\n{string_indent}\"message\\n\"\n{inside}    );\n{inside}rc = 1;\n{nested}}}\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn statement_after_cast_call_in_case_after_long_split_else_keeps_case_body_indent() {
    let depth = 64;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n      case ONE: {\n        if(value!=4){\n          call(stderr,\n            \"message\\n\"\n          );\n          rc = 1;\n          goto exit;\n        }\n        done = 1;\n        size = (int)value(arg[2]);\n        text = arg[3];\n        other = value(text);\n        break;\n      }\n    }\n  }\nexit:\n  return;\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    let inside = " ".repeat((depth + 4) * 4);
    let string_indent = " ".repeat((depth + 4) * 4 + "call(".len());
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}if(value!=4) {{\n{inside}call(stderr,\n{string_indent}\"message\\n\"\n{inside}    );\n{inside}rc = 1;\n{inside}goto exit;\n{nested}}}\n{nested}done = 1;\n{nested}size = (int)value(arg[2]);\n{nested}text = arg[3];\n{nested}other = value(text);\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\nexit:\n    return;\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn block_comment_in_switch_case_after_long_split_else_keeps_case_indent() {
    let depth = 64;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n      case ONE: {\n        first();\n        break;\n      }\n      case TWO: {\n        /* Examples:\n        ** one\n        */\n        int x;\n        done();\n        break;\n      }\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}first();\n{nested}break;\n{body}}}\n{body}case TWO: {{\n{nested}/* Examples:\n{nested}** one\n{nested}*/\n{nested}int x;\n{nested}done();\n{nested}break;\n{body}}}\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn switch_case_after_long_split_else_keeps_case_indent_past_lookback() {
    let depth = 64;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n      case ONE: {\n        first();\n        break;\n      }\n      case TWO:\n        if(flag){\n          done();\n        }\n        break;\n      case THREE:\n        if(flag){\n          more();\n        }\n        break;\n    }\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}first();\n{nested}break;\n{body}}}\n{body}case TWO:\n{nested}if(flag) {{\n{nested}    done();\n{nested}}}\n{nested}break;\n{body}case THREE:\n{nested}if(flag) {{\n{nested}    more();\n{nested}}}\n{nested}break;\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn deep_switch_case_after_long_split_else_keeps_case_body_indent() {
    let depth = 48;
    let mut input =
        String::from("void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    } else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}} else\n\n"
        ));
    }

    input.push_str("  if(t){\n    switch(value){\n      case ONE: {\n        static const struct {\n          int id;\n        } items[] = {\n");
    for index in 1..=35 {
        input.push_str(&format!("          {{{index}}},\n"));
    }
    input.push_str("        };\n        for(i=0; i<n; i++){\n          if(i==0){\n            one();\n          }else if(i==1){\n            two();\n          }else{\n            three();\n          }\n        }\n        break;\n      }\n      /* next */\n      case TWO:\n      case THREE:\n      case FOUR:\n        if(flag){\n          int opt = value();\n          result = call(opt);\n          done = 1;\n        }\n        break;\n    }\n  }\n}\n");

    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let nested = " ".repeat((depth + 3) * 4);
    expected.push_str(&format!(
        "{outer}if(t) {{\n{body}switch(value) {{\n{body}case ONE: {{\n{nested}static const struct {{\n{nested}    int id;\n{nested}}} items[] = {{\n"
    ));
    for index in 1..=35 {
        expected.push_str(&format!("{nested}    {{{index}}},\n"));
    }
    expected.push_str(&format!(
        "{nested}}};\n{nested}for(i=0; i<n; i++) {{\n{nested}    if(i==0) {{\n{nested}        one();\n{nested}    }} else if(i==1) {{\n{nested}        two();\n{nested}    }} else {{\n{nested}        three();\n{nested}    }}\n{nested}}}\n{nested}break;\n{body}}}\n{body}/* next */\n{body}case TWO:\n{body}case THREE:\n{body}case FOUR:\n{nested}if(flag) {{\n{nested}    int opt = value();\n{nested}    result = call(opt);\n{nested}    done = 1;\n{nested}}}\n{nested}break;\n{body}}}\n{outer}}}\n}}\n"
    ));

    check(&input, &[], &expected);
}

#[test]
fn block_comment_in_braced_if_inside_split_body_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    if( d ){\n      /* comment\n      ** tail\n      */\n      call();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                if( d ) {\n                    /* comment\n                    ** tail\n                    */\n                    call();\n                }\n            }\n}\n",
    );
}

#[test]
fn block_comment_in_braced_else_inside_split_body_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    if( x ){\n      one();\n    }else{\n      /* comment\n      ** tail\n      */\n      call();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                if( x ) {\n                    one();\n                } else {\n                    /* comment\n                    ** tail\n                    */\n                    call();\n                }\n            }\n}\n",
    );
}

#[test]
fn single_line_block_comment_in_split_else_if_body_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    if( c ){\n      if( d ){\n        second();\n      }else if( e ){\n        /* comment */\n      }else if( g ){\n        next();\n      }\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            if( c ) {\n                if( d ) {\n                    second();\n                } else if( e ) {\n                    /* comment */\n                } else if( g ) {\n                    next();\n                }\n            }\n        }\n}\n",
    );
}

#[test]
fn multiline_call_argument_after_string_in_deep_split_else_keeps_call_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a0 ){ call0(); }else\n#endif\n\n  if( a1 ){ call1(); }else\n\n  if( a2 ){ call2(); }else\n\n#ifndef OMIT_X\n  if( a3 ){ call3(); }else\n#endif\n\n  if( a4 ){\n    if( safe ){\n      print(out,\n            \"Cannot run command such as \\\"%s\\\" here\\n\",\n            arg[0]);\n      rc = 1;\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a0 ) {\n        call0();\n    } else\n#endif\n\n        if( a1 ) {\n            call1();\n        } else\n\n            if( a2 ) {\n                call2();\n            } else\n\n#ifndef OMIT_X\n                if( a3 ) {\n                    call3();\n                } else\n#endif\n\n                    if( a4 ) {\n                        if( safe ) {\n                            print(out,\n                                  \"Cannot run command such as \\\"%s\\\" here\\n\",\n                                  arg[0]);\n                            rc = 1;\n                        }\n                    }\n}\n",
    );
}

#[test]
fn close_paren_after_adjacent_string_call_in_split_else_keeps_call_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a0 ){ first(); }else\n#endif\n\n  if( a1 ){ second(); }else\n\n  if( b ){\n    make(out,\n         \"alpha\\n\"\n         \"beta\\n\"\n        );\n    if( flag ){\n      call();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a0 ) {\n        first();\n    } else\n#endif\n\n        if( a1 ) {\n            second();\n        } else\n\n            if( b ) {\n                make(out,\n                     \"alpha\\n\"\n                     \"beta\\n\"\n                    );\n                if( flag ) {\n                    call();\n                }\n            }\n}\n",
    );
}

#[test]
fn statement_after_adjacent_string_call_sequence_in_split_else_keeps_block_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a0 ){ first(); }else\n#endif\n\n  if( a1 ){ second(); }else\n\n  if( b ){\n    make(out,\n         \"alpha\\n\"\n         \"beta\\n\"\n        );\n    if( expr ){\n      append(out,\n             \"gamma\\n\", sep);\n      sep = \"AND\";\n    }\n    if( flag ){\n      append(out, \"delta\", sep);\n    }\n    append(out, \"tail\");\n\n    /* comment */\n    if( debug ){\n      print();\n    }else{\n      run();\n    }\n    free(out);\n  }else\n\n  if( c ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a0 ) {\n        first();\n    } else\n#endif\n\n        if( a1 ) {\n            second();\n        } else\n\n            if( b ) {\n                make(out,\n                     \"alpha\\n\"\n                     \"beta\\n\"\n                    );\n                if( expr ) {\n                    append(out,\n                           \"gamma\\n\", sep);\n                    sep = \"AND\";\n                }\n                if( flag ) {\n                    append(out, \"delta\", sep);\n                }\n                append(out, \"tail\");\n\n                /* comment */\n                if( debug ) {\n                    print();\n                } else {\n                    run();\n                }\n                free(out);\n            } else\n\n                if( c ) {\n                    next();\n                }\n}\n",
    );
}

#[test]
fn block_comment_before_assignment_call_in_preprocessor_branch_uses_value_indent() {
    check(
        "void f(void){\n  if( first ){\n    one();\n  }else\n\n  if( second ){\n    char *text = /* note */\n      \"alpha\"\n      \"beta\";\n#if FEATURE\n    {\n      text = call(text, flag ? \"\" : \"suffix\");\n      text = call(\n          /* call note */\n          \"gamma\"\n          \"delta\"\n          , text);\n      check(text);\n    }\n#endif\n  }\n}\n",
        &[],
        "void f(void) {\n    if( first ) {\n        one();\n    } else\n\n        if( second ) {\n            char *text = /* note */\n                \"alpha\"\n                \"beta\";\n#if FEATURE\n            {\n                text = call(text, flag ? \"\" : \"suffix\");\n                text = call(\n                           /* call note */\n                           \"gamma\"\n                           \"delta\"\n                           , text);\n                check(text);\n            }\n#endif\n        }\n}\n",
    );
}

#[test]
fn block_comment_before_adjacent_string_call_in_split_else_uses_call_indent() {
    check(
        "void f(void){\n  if( a0 ){ one(); }else\n\n  if( a1 ){ one(); }else\n\n  if( a2 ){ one(); }else\n\n  if( a3 ){ one(); }else\n\n  if( b ){\n    value = call(\n      /* note */\n      \"alpha\"\n      , other);\n  }\n}\n",
        &[],
        "void f(void) {\n    if( a0 ) {\n        one();\n    } else\n\n        if( a1 ) {\n            one();\n        } else\n\n            if( a2 ) {\n                one();\n            } else\n\n                if( a3 ) {\n                    one();\n                } else\n\n                    if( b ) {\n                        value = call(\n                                    /* note */\n                                    \"alpha\"\n                                    , other);\n                    }\n}\n",
    );
}

#[test]
fn multiline_string_call_argument_in_split_else_keeps_call_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    if( d ){\n      value = make(\n        \"alpha\"\n        \"beta\",\n        arg\n      );\n      call();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                if( d ) {\n                    value = make(\n                                \"alpha\"\n                                \"beta\",\n                                arg\n                            );\n                    call();\n                }\n            }\n}\n",
    );
}

#[test]
fn multiline_string_call_in_braced_split_else_keeps_call_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    if( d ){\n      third();\n    }else{\n      char *value = make(\n          \"alpha\"\n          \"beta\"\n          \"gamma\", arg\n      );\n\n      if( value ){\n        use(value);\n      }\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                if( d ) {\n                    third();\n                } else {\n                    char *value = make(\n                                      \"alpha\"\n                                      \"beta\"\n                                      \"gamma\", arg\n                                  );\n\n                    if( value ) {\n                        use(value);\n                    }\n                }\n            }\n}\n",
    );
}

#[test]
fn preprocessor_split_else_if_in_deep_split_body_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    if( x ){\n      one();\n    }else if( y ){\n      two();\n#ifdef DEBUG\n    }else if( z ){\n      three();\n#endif\n    }else{\n      four();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                if( x ) {\n                    one();\n                } else if( y ) {\n                    two();\n#ifdef DEBUG\n                } else if( z ) {\n                    three();\n#endif\n                } else {\n                    four();\n                }\n            }\n}\n",
    );
}

#[test]
fn block_comment_after_preprocessor_split_else_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n#ifndef OMIT_X\n  if( c ){\n    third();\n  }else\n#endif\n\n  /* comment\n  ** tail */\n  if( d ){\n    fourth();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n#ifndef OMIT_X\n            if( c ) {\n                third();\n            } else\n#endif\n\n                /* comment\n                ** tail */\n                if( d ) {\n                    fourth();\n                }\n}\n",
    );
}

#[test]
fn preprocessor_branch_in_deep_split_else_body_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    for(i=0; i<n; i++){\n      if( arg[i] ){\n#ifdef FEATURE\n        error(out, \"bad\",\n          \"more\");\n        rc = 1;\n#else\n        set();\n#endif\n      }\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                for(i=0; i<n; i++) {\n                    if( arg[i] ) {\n#ifdef FEATURE\n                        error(out, \"bad\",\n                              \"more\");\n                        rc = 1;\n#else\n                        set();\n#endif\n                    }\n                }\n            }\n}\n",
    );
}

#[test]
fn preprocessor_condition_after_split_else_loop_uses_closing_header_body_indent() {
    check(
        "void f(void){\n  for(i=0; i<n; i++){\n    const char *value = args[i];\n#ifndef A\n    if( option(value) ){\n      flag = 1;\n    }else\n#endif\n    if( value[0]=='-' ){\n      error();\n      rc = 1;\n      goto done;\n    }else if( name ){\n      error();\n      rc = 1;\n      goto done;\n    }else{\n      name = value;\n    }\n  }\n\n  close_all();\n  db = 0;\n  mode = mode;\n\n  if( name || mode==HEX ){\n    if( fresh && name && !safe ){\n      if( prefix(name) ){\n        char *del = uri(name);\n        check(del);\n        delete(del);\n        free(del);\n      }else{\n        delete(name);\n      }\n    }\n#ifndef A\n    if( safe\n     && mode!=HEX\n     && name\n     && compare(name,\":memory:\")!=0\n    ){\n      fail();\n    }\n#else\n    /* comment */\n#endif\n    if( name ){\n      next();\n    }\n  }\ndone:\n}\n",
        &[],
        "void f(void) {\n    for(i=0; i<n; i++) {\n        const char *value = args[i];\n#ifndef A\n        if( option(value) ) {\n            flag = 1;\n        } else\n#endif\n            if( value[0]=='-' ) {\n                error();\n                rc = 1;\n                goto done;\n            } else if( name ) {\n                error();\n                rc = 1;\n                goto done;\n            } else {\n                name = value;\n            }\n    }\n\n    close_all();\n    db = 0;\n    mode = mode;\n\n    if( name || mode==HEX ) {\n        if( fresh && name && !safe ) {\n            if( prefix(name) ) {\n                char *del = uri(name);\n                check(del);\n                delete(del);\n                free(del);\n            } else {\n                delete(name);\n            }\n        }\n#ifndef A\n        if( safe\n                && mode!=HEX\n                && name\n                && compare(name,\":memory:\")!=0\n          ) {\n            fail();\n        }\n#else\n        /* comment */\n#endif\n        if( name ) {\n            next();\n        }\n    }\ndone:\n}\n",
    );
}

#[test]
fn local_struct_array_in_deep_split_else_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  /* comment */\n  if( b ){\n    second();\n  }else\n\n#ifndef OMIT_X\n  if( c ){\n    third();\n  }else\n#endif\n\n  if( d ){\n    for(i=0; i<n; i++){\n      print(out, \"%s %s\",\n            name, value ? \"yes\" : \"no\");\n      free(name);\n    }\n  }else\n\n  if( e ){\n    static const struct Choice {\n      const char *name;\n      int op;\n    } items[] = {\n      { \"one\", 1 },\n      { \"two\", 2 },\n    };\n    int i;\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        /* comment */\n        if( b ) {\n            second();\n        } else\n\n#ifndef OMIT_X\n            if( c ) {\n                third();\n            } else\n#endif\n\n                if( d ) {\n                    for(i=0; i<n; i++) {\n                        print(out, \"%s %s\",\n                              name, value ? \"yes\" : \"no\");\n                        free(name);\n                    }\n                } else\n\n                    if( e ) {\n                        static const struct Choice {\n                            const char *name;\n                            int op;\n                        } items[] = {\n                            { \"one\", 1 },\n                            { \"two\", 2 },\n                        };\n                        int i;\n                        call();\n                    }\n}\n",
    );
}

#[test]
fn multiline_call_in_split_else_keeps_following_loop_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a ){\n    first();\n  }else\n#endif\n\n  if( b ){\n    second();\n  }else\n\n  if( c ){\n    clear(flag,\n       left|right|other);\n    for(i=0; i<n; i++){\n      if( arg[i] ){\n        call();\n      }\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a ) {\n        first();\n    } else\n#endif\n\n        if( b ) {\n            second();\n        } else\n\n            if( c ) {\n                clear(flag,\n                      left|right|other);\n                for(i=0; i<n; i++) {\n                    if( arg[i] ) {\n                        call();\n                    }\n                }\n            }\n}\n",
    );
}

#[test]
fn local_struct_in_deep_split_else_chain_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a0 ){\n    call0();\n  }else\n#endif\n\n  if( a1 ){\n    call1();\n  }else\n\n  if( a2 ){\n    call2();\n  }else\n\n#ifndef OMIT_X\n  if( a3 ){\n    call3();\n  }else\n#endif\n\n  if( a4 ){\n    call4();\n  }else\n\n  if( a5 ){\n    call5();\n  }else\n\n  if( a6 ){\n    call6();\n  }else\n\n  if( a7 ){\n    call7();\n  }else\n\n  if( e ){\n    static const struct Choice {\n      const char *name;\n      int op;\n    } items[] = {\n      { \"one\", 1 },\n      { \"two\", 2 },\n    };\n    int i;\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a0 ) {\n        call0();\n    } else\n#endif\n\n        if( a1 ) {\n            call1();\n        } else\n\n            if( a2 ) {\n                call2();\n            } else\n\n#ifndef OMIT_X\n                if( a3 ) {\n                    call3();\n                } else\n#endif\n\n                    if( a4 ) {\n                        call4();\n                    } else\n\n                        if( a5 ) {\n                            call5();\n                        } else\n\n                            if( a6 ) {\n                                call6();\n                            } else\n\n                                if( a7 ) {\n                                    call7();\n                                } else\n\n                                    if( e ) {\n                                        static const struct Choice {\n                                            const char *name;\n                                            int op;\n                                        } items[] = {\n                                            { \"one\", 1 },\n                                            { \"two\", 2 },\n                                        };\n                                        int i;\n                                        call();\n                                    }\n}\n",
    );
}

#[test]
fn close_after_nested_braceless_else_body_keeps_parent_indent() {
    check(
        "char *f(int *db, int *renamed, int hasDupes){\n  if( outer ){\n    return 0;\n  }else{\n    char *result = 0;\n    if( guarded ){\n#ifdef CLEAN\n      prepare();\n#endif\n      finish();\n    }\n    if( renamed!=0 ){\n      if( !hasDupes ) *renamed = 0;\n      else{\n        finalize(stmt);\n        if( prepare(*db, value, -1, &stmt, 0)\n            && step(stmt) ){\n          *renamed = make();\n        }else\n          *renamed = 0;\n      }\n    }\n    finalize(stmt);\n    close(*db);\n    *db = 0;\n    return result;\n  }\n}\n",
        &[],
        "char *f(int *db, int *renamed, int hasDupes) {\n    if( outer ) {\n        return 0;\n    } else {\n        char *result = 0;\n        if( guarded ) {\n#ifdef CLEAN\n            prepare();\n#endif\n            finish();\n        }\n        if( renamed!=0 ) {\n            if( !hasDupes ) *renamed = 0;\n            else {\n                finalize(stmt);\n                if( prepare(*db, value, -1, &stmt, 0)\n                        && step(stmt) ) {\n                    *renamed = make();\n                } else\n                    *renamed = 0;\n            }\n        }\n        finalize(stmt);\n        close(*db);\n        *db = 0;\n        return result;\n    }\n}\n",
    );
}

#[test]
fn closing_condition_after_preprocessor_braceless_else_uses_condition_indent() {
    check(
        "void f(int c, int n){\n#if FEATURE\n  if( first ){\n    done();\n  }else\n#endif\n\n#ifndef OMIT_X\n  if( (c=='b' && n>=3 && call(a, \"backup\", n)==0)\n   || (c=='s' && n>=3 && call(a, \"save\", n)==0)\n  ){\n    done();\n  }\n#endif\n}\n",
        &[],
        "void f(int c, int n) {\n#if FEATURE\n    if( first ) {\n        done();\n    } else\n#endif\n\n#ifndef OMIT_X\n        if( (c=='b' && n>=3 && call(a, \"backup\", n)==0)\n                || (c=='s' && n>=3 && call(a, \"save\", n)==0)\n          ) {\n            done();\n        }\n#endif\n}\n",
    );
}

#[test]
fn preprocessor_branch_after_nested_braceless_else_uses_else_body_indent() {
    check(
        "void f(int n){\n  if( first ){\n    done();\n  }else\n\n  /* comment */\n  if( second ){\n    call();\n  }else\n\n  /* next comment\n  ** tail\n  */\n  if( third ){\n    go();\n  }else\n\n#ifndef OMIT_X\n  if( fourth ){\n    more();\n  }\n#endif\n}\n",
        &[],
        "void f(int n) {\n    if( first ) {\n        done();\n    } else\n\n        /* comment */\n        if( second ) {\n            call();\n        } else\n\n            /* next comment\n            ** tail\n            */\n            if( third ) {\n                go();\n            } else\n\n#ifndef OMIT_X\n                if( fourth ) {\n                    more();\n                }\n#endif\n}\n",
    );
}

#[test]
fn statement_after_preprocessor_continuation_keeps_block_indent() {
    check(
        "void f(void){\n#ifdef A\n  call(\"x\",\n       value);\n#endif\n\n  if( ok ){\n    done();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifdef A\n    call(\"x\",\n         value);\n#endif\n\n    if( ok ) {\n        done();\n    }\n}\n",
    );
}

#[test]
fn prior_preprocessor_comments_do_not_extend_braceless_else_body() {
    check(
        "#if A\n#define VALUE 1\n#endif\n\n/* comment */\n#define LIMIT 2\n\nvoid f(void){\n  if( a ){\n    first();\n  }\n  else if( b ){\n    if( c )\n      one();\n    else\n      two();\n    done();\n  }\n}\n",
        &[],
        "#if A\n#define VALUE 1\n#endif\n\n/* comment */\n#define LIMIT 2\n\nvoid f(void) {\n    if( a ) {\n        first();\n    }\n    else if( b ) {\n        if( c )\n            one();\n        else\n            two();\n        done();\n    }\n}\n",
    );
}

#[test]
fn braceless_else_body_after_endif_blank_uses_else_indent() {
    check(
        "void f(void){\n  if( a ){\n    first();\n  }else\n#ifndef A\n  if( b ){\n    second();\n  }else\n#endif\n\n  if( c ){\n    third();\n  }\n}\n",
        &[],
        "void f(void) {\n    if( a ) {\n        first();\n    } else\n#ifndef A\n        if( b ) {\n            second();\n        } else\n#endif\n\n            if( c ) {\n                third();\n            }\n}\n",
    );
}

#[test]
fn comment_after_endif_braceless_else_keeps_nested_body_indent() {
    check(
        "void f(void){\n#ifndef A\n  if( a ){\n    done();\n  }else\n#endif\n\n  if( b ){\n    first();\n  }else\n\n  /* comment */\n  if( c ){\n    second();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef A\n    if( a ) {\n        done();\n    } else\n#endif\n\n        if( b ) {\n            first();\n        } else\n\n            /* comment */\n            if( c ) {\n                second();\n            }\n}\n",
    );
}

#[test]
fn split_else_if_inside_deep_endif_braceless_chain_uses_header_body_indent() {
    check(
        "void f(void){\n#ifndef A\n  if( a ){\n    done();\n  }else\n#endif\n\n  if( b ){\n    first();\n  }else\n\n  /* comment */\n  if( c ){\n    second();\n  }else\n\n#ifndef A\n  if( d ){\n    call();\n  }else\n#endif\n\n  if( e ){\n    if( n==1 ){\n      one();\n    }else if( n==3\n           && ok() ){\n      int i = 0;\n      use(i);\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef A\n    if( a ) {\n        done();\n    } else\n#endif\n\n        if( b ) {\n            first();\n        } else\n\n            /* comment */\n            if( c ) {\n                second();\n            } else\n\n#ifndef A\n                if( d ) {\n                    call();\n                } else\n#endif\n\n                    if( e ) {\n                        if( n==1 ) {\n                            one();\n                        } else if( n==3\n                                   && ok() ) {\n                            int i = 0;\n                            use(i);\n                        }\n                    }\n}\n",
    );
}

#[test]
fn block_comment_inside_deep_endif_braceless_chain_uses_block_body_indent() {
    check(
        "void f(void){\n#ifndef A\n  if( a ){\n    done();\n  }else\n#endif\n\n  if( b ){\n    first();\n  }else\n\n  /* comment */\n  if( c ){\n    second();\n  }else\n\n#ifndef A\n  if( d ){\n    call();\n  }else\n#endif\n\n  if( e ){\n    if( n==1 ){\n      /* list */\n      int i;\n      for(i=0; i<n; i++){\n        use(i);\n      }\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef A\n    if( a ) {\n        done();\n    } else\n#endif\n\n        if( b ) {\n            first();\n        } else\n\n            /* comment */\n            if( c ) {\n                second();\n            } else\n\n#ifndef A\n                if( d ) {\n                    call();\n                } else\n#endif\n\n                    if( e ) {\n                        if( n==1 ) {\n                            /* list */\n                            int i;\n                            for(i=0; i<n; i++) {\n                                use(i);\n                            }\n                        }\n                    }\n}\n",
    );
}

#[test]
fn nested_preprocessor_body_after_endif_braceless_chain_keeps_block_indent() {
    check(
        "void f(void){\n#ifndef A\n  if( a ){\n    if( ok ){\n      done();\n    }\n  }else\n#endif\n\n  if( b ){\n    first();\n  }else\n\n  /* comment */\n  if( c ){\n    second();\n  }else\n\n#ifndef A\n  if( d ){\n    call();\n    if( n==2 ){\n#if X\n      char *z = make(arg);\n      rc = !set(z);\n      free(z);\n#else\n      rc = change(arg);\n#endif\n      if( rc ){\n        error();\n        rc = 1;\n      }\n    }else{\n      usage();\n      rc = 1;\n    }\n  }else\n#endif\n\n  if( e ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef A\n    if( a ) {\n        if( ok ) {\n            done();\n        }\n    } else\n#endif\n\n        if( b ) {\n            first();\n        } else\n\n            /* comment */\n            if( c ) {\n                second();\n            } else\n\n#ifndef A\n                if( d ) {\n                    call();\n                    if( n==2 ) {\n#if X\n                        char *z = make(arg);\n                        rc = !set(z);\n                        free(z);\n#else\n                        rc = change(arg);\n#endif\n                        if( rc ) {\n                            error();\n                            rc = 1;\n                        }\n                    } else {\n                        usage();\n                        rc = 1;\n                    }\n                } else\n#endif\n\n                    if( e ) {\n                        next();\n                    }\n}\n",
    );
}

#[test]
fn block_comment_after_preprocessor_else_uses_body_indent() {
    check(
        "void f(void){\n  if( enabled ){\n#ifdef A\n    call(\"x\",\n         value);\n#else\n    /* comment */\n    done();\n#endif\n  }\n}\n",
        &[],
        "void f(void) {\n    if( enabled ) {\n#ifdef A\n        call(\"x\",\n             value);\n#else\n        /* comment */\n        done();\n#endif\n    }\n}\n",
    );
}

#[test]
fn nested_preprocessor_branch_inside_braceless_else_keeps_block_indent() {
    check(
        "void f(int n){\n  if( first ){\n    done();\n  }else\n\n  /* comment */\n  if( second ){\n    call();\n  }else\n\n  /* next comment\n  ** tail\n  */\n  if( third ){\n    go();\n  }else\n\n#ifndef OMIT_X\n  if( fourth ){\n    if( n ){\n#if A\n      alpha();\n#else\n      beta();\n#endif\n      if( done ){\n        ok();\n      }\n    }\n  }\n#endif\n}\n",
        &[],
        "void f(int n) {\n    if( first ) {\n        done();\n    } else\n\n        /* comment */\n        if( second ) {\n            call();\n        } else\n\n            /* next comment\n            ** tail\n            */\n            if( third ) {\n                go();\n            } else\n\n#ifndef OMIT_X\n                if( fourth ) {\n                    if( n ) {\n#if A\n                        alpha();\n#else\n                        beta();\n#endif\n                        if( done ) {\n                            ok();\n                        }\n                    }\n                }\n#endif\n}\n",
    );
}

#[test]
fn function_after_format_macro_comments_stays_unindented() {
    let source = "/*\n** first setting\n*/\n#define FIRST_VALUE (1 + helper(MAX_VALUE))\n\n\n/*\n** second setting\n*/\n#define SECOND_VALUE 2\n\n\n#if !defined(FLAGS)\n\n/* option flags */\n#define FLAGS \"-+\"\n\n#endif\n\n\n/*\n** final setting\n*/\n#define LIMIT 32\n\n\nstatic void f(void) {\n}\n";

    check(source, &[], source);
}

#[test]
fn user_label_block_inside_case_closes_at_body_indent() {
    check(
        "void f(int value) {\n  switch (value) {\n    case 1:\n      flag = A;\n      goto target;\n    case 2:\n      flag = B;\ntarget: {\n        int result = value;\n        call(result);\n        break;\n      }\n    case 3:\n      call(value);\n  }\n}\n",
        &[],
        "void f(int value) {\n    switch (value) {\n    case 1:\n        flag = A;\n        goto target;\n    case 2:\n        flag = B;\ntarget: {\n            int result = value;\n            call(result);\n            break;\n        }\n    case 3:\n        call(value);\n    }\n}\n",
    );
}

#[test]
fn nested_branches_in_case_user_label_block_keep_label_body_indent() {
    check(
        "void f(int value){\n  switch(value){\n    default: target: {\n      int result = value;\n      if (result) {\n        if (value) {\n          result = 1; goto done;\n        }\n        else\n          result = 0;\n      }\n      else {\n        call(result);\n      }\n      break;\n    }\n  }\ndone:\n  return;\n}\n",
        &[],
        "void f(int value) {\n    switch(value) {\n    default:\ntarget: {\n            int result = value;\n            if (result) {\n                if (value) {\n                    result = 1;\n                    goto done;\n                }\n                else\n                    result = 0;\n            }\n            else {\n                call(result);\n            }\n            break;\n        }\n    }\ndone:\n    return;\n}\n",
    );
}

#[test]
fn nested_case_else_block_in_user_label_keeps_block_indent() {
    check(
        "void f(int value){\n  switch(value){\n    default: target: {\n      switch(value){\n        case 1: {\n          if (value)\n            call();\n          else {\n            value = 1; goto done;\n          }\n          break;\n        }\n      }\n      break;\n    }\n  }\ndone:\n  return;\n}\n",
        &[],
        "void f(int value) {\n    switch(value) {\n    default:\ntarget: {\n            switch(value) {\n            case 1: {\n                if (value)\n                    call();\n                else {\n                    value = 1;\n                    goto done;\n                }\n                break;\n            }\n            }\n            break;\n        }\n    }\ndone:\n    return;\n}\n",
    );
}

#[test]
fn casted_call_assignment_inside_case_uses_cast_indent_for_argument() {
    check(
        "void f(int value) {\n  switch (value) {\n    case TEXT_ID: {\n      Value_Number len = (Value_Number)readvalue(c, data + off,\n                                           c.is_small, to_value(size), 0);\n      call(len);\n      break;\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n    switch (value) {\n    case TEXT_ID: {\n        Value_Number len = (Value_Number)readvalue(c, data + off,\n                           c.is_small, to_value(size), 0);\n        call(len);\n        break;\n    }\n    }\n}\n",
    );
}

#[test]
fn call_argument_trailing_return_lambda_brace_attaches_idempotently() {
    let attached =
        "void f()\n{\n    foo(baz,\n    [this]() -> bool {\n        return g();\n    });\n}\n";
    let broken = "void f()\n{\n    foo(baz,\n        [this]() -> bool\n        {\n            return g();\n        });\n}\n";
    check(broken, &["--style=1tbs"], attached);
    check(attached, &["--style=1tbs"], attached);
}

// Sibling call arguments and constructor members retain their own owner columns.
#[test]
fn member_init_list_stays_consistent_after_nested_lambda_close_brace() {
    check(
        "Type::Type()\n    : m_alpha(makeAlpha(\n                  [this]() {\n                  return ready();\n},\n                  extra))\n, m_beta(new Beta)\n, m_gamma(new Gamma)\n{\n}\n",
        &["--style=1tbs", "--min-conditional-indent=0"],
        "Type::Type()\n    : m_alpha(makeAlpha(\n              [this]()\n{\n    return ready();\n},\n              extra))\n    , m_beta(new Beta)\n    , m_gamma(new Gamma)\n{\n}\n",
    );
}

#[test]
fn combined_c_options_keep_colon_unpadded_in_initializer_pointer_expression() {
    check(
        "items[] = { red,\n            green:\n            ^          *blue\n          };\n",
        COMBINED_C_ARGS,
        "items[] = { red,\n            green:\n            ^          *blue\n          };\n",
    );
}

#[test]
fn lambda_chained_call_after_nested_call_aligns_to_outer_open_paren() {
    check(
        "void f()\n{\n    auto callbackAction = [this, context](int key) {\n        context->targetValue->replace(id(\"Selected Generic Option: %1\")\n                                      .arg(context->selector->currentItem()));\n    };\n}\n",
        &[],
        "void f()\n{\n    auto callbackAction = [this, context](int key) {\n        context->targetValue->replace(id(\"Selected Generic Option: %1\")\n                                      .arg(context->selector->currentItem()));\n    };\n}\n",
    );
}

#[test]
fn lambda_call_argument_after_open_paren_uses_body_continuation_indent() {
    check(
        "void f()\n{\n    start().onFailed([promise] {\n        const auto ex = std::make_exception_ptr(\n                    std::runtime_error(\"Unknown error occurred while processing.\"));\n        promise->setException(ex);\n    });\n}\n",
        &[],
        "void f()\n{\n    start().onFailed([promise] {\n        const auto ex = std::make_exception_ptr(\n            std::runtime_error(\"Unknown error occurred while processing.\"));\n        promise->setException(ex);\n    });\n}\n",
    );
}

#[test]
fn lambda_chain_nested_call_argument_keeps_outer_call_column() {
    check(
        "void f()\n{\n        future\n                .then([this](auto) {\n                    watcher.setFuture(VeryLongNamespace::run(Task::scaled,\n                                                             future.results()));\n                });\n}\n",
        &[],
        "void f()\n{\n    future\n    .then([this](auto) {\n        watcher.setFuture(VeryLongNamespace::run(Task::scaled,\n                          future.results()));\n    });\n}\n",
    );
}

#[test]
fn case_logical_if_chain_keeps_operator_indent_after_call_operand() {
    check(
        "void f()\n{\n    switch (type) {\n        case ValueRequest: {\n            for (int i = 0; i < pendingValues.size(); ++i) {\n                if (valueInfo.entryIndex == to_i32(index)\n                    && valueInfo.offset == to_i32(start)\n                    && valueInfo.length == to_i32(length)) {\n                    call();\n                }\n            }\n        }\n    }\n}\n",
        &[],
        "void f()\n{\n    switch (type) {\n    case ValueRequest: {\n        for (int i = 0; i < pendingValues.size(); ++i) {\n            if (valueInfo.entryIndex == to_i32(index)\n                    && valueInfo.offset == to_i32(start)\n                    && valueInfo.length == to_i32(length)) {\n                call();\n            }\n        }\n    }\n    }\n}\n",
    );
}

#[test]
fn condition_member_call_after_closed_call_uses_two_level_indent() {
    check(
        "void f()\n{\n    if (Line(event->pos(), event->start())\n        .length() < start()) {\n        return;\n    }\n}\n",
        &[],
        "void f()\n{\n    if (Line(event->pos(), event->start())\n            .length() < start()) {\n        return;\n    }\n}\n",
    );
}

#[test]
fn lambda_return_logical_tail_aligns_to_return_value() {
    check(
        "void f()\n{\n    const auto client = find(items.begin(), items.end(),\n                             [&](const Item &item){\n        return item.peerAddress() == peerAddress\n               && item.peerPort() == peerPort;\n    });\n}\n",
        &[],
        "void f()\n{\n    const auto client = find(items.begin(), items.end(),\n    [&](const Item &item) {\n        return item.peerAddress() == peerAddress\n               && item.peerPort() == peerPort;\n    });\n}\n",
    );
}

#[test]
fn call_argument_continuation_realigned_after_cast_and_nested_call() {
    check(
        "void f(void) {\n  switch(type){\n    case TEXT: {\n      call_text(target, index,\n        (const char*)value(),\n        -1, STATIC_VALUE);\n      break;\n    }\n    case BLOB: {\n      call_blob(target, index, blob(source, i),\n                                            bytes(source, i),\n                                            STATIC_VALUE);\n      break;\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n    switch(type) {\n    case TEXT: {\n        call_text(target, index,\n                  (const char*)value(),\n                  -1, STATIC_VALUE);\n        break;\n    }\n    case BLOB: {\n        call_blob(target, index, blob(source, i),\n                  bytes(source, i),\n                  STATIC_VALUE);\n        break;\n    }\n    }\n}\n",
    );
}

#[test]
fn macro_catch_after_one_line_try_keeps_source_gap() {
    check(
        "void f()\n{\n    DO_TRY { CHECK_EQ(value, expected); } DO_CATCH(...) {} // log\n}\n",
        &[],
        "void f()\n{\n    DO_TRY { CHECK_EQ(value, expected); } DO_CATCH(...) {} // log\n}\n",
    );
}

const COMBINED_C_ARGS: &[&str] = &[
    "--style=kr",
    "--mode=c",
    "--indent=spaces=4",
    "--indent-switches",
    "--indent-preprocessor",
    "--indent-preproc-define",
    "--indent-col1-comments",
    "--pad-oper",
    "--pad-comma",
    "--pad-header",
    "--unpad-paren",
    "--break-one-line-headers",
    "--keep-one-line-blocks",
    "--keep-one-line-statements",
    "--align-pointer=name",
    "--align-reference=name",
    "--min-conditional-indent=0",
    "--attach-closing-while",
    "--attach-return-type",
    "--attach-return-type-decl",
    "--convert-tabs",
    "--max-continuation-indent=80",
    "--max-code-length=100",
    "--break-after-logical",
];

#[test]
fn trivial_copy_paths_preserve_non_whitespace_tokens() {
    for source in [
        fixture!(
            "int f(){/* keep { } */char*s=\"/* not a comment */\";char*t=\"// not a comment\";char c='/';return 0;}// tail }"
        ),
        fixture!("const char*s=\"a\\", "/* not a comment */\";",),
        fixture!(
            "#define BODY(x) \\",
            "/* keep */ \\",
            "do { call(x); } while (0)",
            "int y=BODY(1);",
        ),
    ] {
        let actual = format(source);

        assert_eq!(non_whitespace(&actual), non_whitespace(source), "{source}");
    }
}

const TEST_SAMPLE_OPTIONS: &[&str] = &[
    "--style=1tbs",
    "--mode=c",
    "--lineend=linux",
    "--convert-tabs",
    "--indent=spaces=4",
    "--indent-switches",
    "--indent-preprocessor",
    "--indent-preproc-define",
    "--add-braces",
    "--pad-oper",
    "--pad-comma",
    "--pad-header",
    "--unpad-paren",
    "--break-one-line-headers",
    "--break-after-logical",
    "--align-pointer=name",
    "--attach-closing-while",
    "--attach-return-type",
    "--attach-return-type-decl",
    "--min-conditional-indent=0",
    "--max-continuation-indent=80",
    "--max-code-length=109",
];

#[test]
fn logical_call_chain_in_return_keeps_operand_continuation_indent() {
    let expected = "static bool repro(void)\n{\n    if (condition) {\n        return first_call(\n                   one, two, three\n               ) &&\n               second_call(\n                   four, five, six\n               ) &&\n               third_call(seven);\n    }\n    return false;\n}\n";
    check(expected, TEST_SAMPLE_OPTIONS, expected);

    let mut options = FormatOptions::default();
    let args: Vec<String> = TEST_SAMPLE_OPTIONS
        .iter()
        .map(|arg| (*arg).to_string())
        .collect();
    apply_command_line_args(&mut options, &args).expect("valid options");
    let once =
        String::from_utf8(format_bytes(expected.as_bytes(), &options).expect("format bytes"))
            .expect("utf8");
    let twice = String::from_utf8(format_bytes(once.as_bytes(), &options).expect("format bytes"))
        .expect("utf8");

    assert_eq!(twice, once);
    assert!(once.lines().all(|line| line.len() <= 109));
}

#[test]
fn logical_call_chain_in_assignment_keeps_operand_continuation_indent() {
    check(
        "static bool repro(void)\n{\n    bool result;\n    result = first_call(\n                 one, two, three\n             ) ||\n             second_call(\n                 four, five, six\n             ) ||\n             third_call(seven);\n    return result;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static bool repro(void)\n{\n    bool result;\n    result = first_call(\n                 one, two, three\n             ) ||\n             second_call(\n                 four, five, six\n             ) ||\n             third_call(seven);\n    return result;\n}\n",
    );
}

#[test]
fn logical_call_chain_in_control_condition_keeps_operand_continuation_indent() {
    check(
        "static bool repro(void)\n{\n    if (first_call(\n            one, two, three\n        ) &&\n        second_call(\n            four, five, six\n        ) &&\n        third_call(seven)) {\n        return true;\n    }\n    return false;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static bool repro(void)\n{\n    if (first_call(\n            one, two, three\n        ) &&\n        second_call(\n            four, five, six\n        ) &&\n        third_call(seven)) {\n        return true;\n    }\n    return false;\n}\n",
    );
}

#[test]
fn logical_call_chain_with_comment_keeps_operand_continuation_indent() {
    check(
        "static bool repro(void)\n{\n    if (condition) {\n        return first_call(\n                   one, two, three\n               ) &&\n               /* next operand */\n               second_call(\n                   four, five, six\n               ) &&\n               third_call(seven);\n    }\n    return false;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static bool repro(void)\n{\n    if (condition) {\n        return first_call(\n                   one, two, three\n               ) &&\n               /* next operand */\n               second_call(\n                   four, five, six\n               ) &&\n               third_call(seven);\n    }\n    return false;\n}\n",
    );
}

#[test]
fn return_ternary_after_multiline_function_head_aligns_to_value_column() {
    check(
        "static enum result choose(\n    enum result value\n)\n{\n    return value == ZERO ? ONE :\n    TWO;\n}\n",
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--indent=spaces=4",
            "--pad-oper",
            "--break-after-logical",
        ],
        "static enum result choose(\n    enum result value\n)\n{\n    return value == ZERO ? ONE :\n           TWO;\n}\n",
    );
}

#[test]
fn statement_braces_attach_after_multiline_parameter_function() {
    check(
        "static bool choose(\n    enum target target,\n    enum field *field\n)\n{\n    if (field == nullptr)\n    {\n        return false;\n    }\n    switch (target)\n    {\n    case ZERO:\n        return true;\n    default:\n        return false;\n    }\n}\n",
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--indent=spaces=4",
        ],
        "static bool choose(\n    enum target target,\n    enum field *field\n)\n{\n    if (field == nullptr) {\n        return false;\n    }\n    switch (target) {\n    case ZERO:\n        return true;\n    default:\n        return false;\n    }\n}\n",
    );
}

#[test]
fn bracket_continuation_aligns_to_opening_bracket_context() {
    check(
        "void f(void)\n{\n    uint8_t value = table[\n                    index + 1u\n                ];\n    use(value);\n}\n",
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--indent=spaces=4",
            "--pad-oper",
        ],
        "void f(void)\n{\n    uint8_t value = table[\n                        index + 1u\n                    ];\n    use(value);\n}\n",
    );
}

#[test]
fn initializer_member_ternary_arm_aligns_to_value_column() {
    check(
        "struct command\n{\n    uint8_t sequence;\n    uint8_t measure;\n};\nvoid f(struct command *value, uint8_t uses_measure, uint8_t measure_sequence)\n{\n    *value = (struct command) {\n        .sequence = uses_measure ?\n                  measure_sequence : 0u,\n        .measure = 1u,\n    };\n}\n",
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--indent=spaces=4",
            "--pad-oper",
            "--break-after-logical",
        ],
        "struct command {\n    uint8_t sequence;\n    uint8_t measure;\n};\nvoid f(struct command *value, uint8_t uses_measure, uint8_t measure_sequence)\n{\n    *value = (struct command) {\n        .sequence = uses_measure ?\n                    measure_sequence : 0u,\n        .measure = 1u,\n    };\n}\n",
    );
}

#[test]
fn ternary_arm_after_closing_the_condition_group_aligns_with_its_paren() {
    check(
        "void f(void)\n{\n    const struct clock_value time = (view != nullptr &&\n                                     view->clock.valid &&\n                                     view->clock.rtc_valid &&\n                                     clock_value_valid(&view->clock.time)) ?\n                                     view->clock.time : default_time;\n    use(time);\n}\n",
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--indent=spaces=4",
            "--pad-oper",
            "--break-after-logical",
        ],
        "void f(void)\n{\n    const struct clock_value time = (view != nullptr &&\n                                     view->clock.valid &&\n                                     view->clock.rtc_valid &&\n                                     clock_value_valid(&view->clock.time)) ?\n                                    view->clock.time : default_time;\n    use(time);\n}\n",
    );
}

#[test]
fn pad_oper_keeps_right_padding_for_macro_like_operands_on_continuation_lines() {
    check(
        "static bool check(uint32_t count, uint32_t mask, struct Item item)\n{\n    const size_t scaled =\n        (size_t)count * MACRO_SCALE;\n    const bool ready =\n        MACRO_READY && item.value == MACRO_VALUE;\n    const uint32_t bits =\n        mask ^ MACRO_MASK;\n    const uint32_t flags =\n        mask & MACRO_FLAGS;\n    return ready;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static bool check(uint32_t count, uint32_t mask, struct Item item)\n{\n    const size_t scaled =\n        (size_t)count * MACRO_SCALE;\n    const bool ready =\n        MACRO_READY && item.value == MACRO_VALUE;\n    const uint32_t bits =\n        mask ^ MACRO_MASK;\n    const uint32_t flags =\n        mask & MACRO_FLAGS;\n    return ready;\n}\n",
    );
}

#[test]
fn pad_oper_keeps_star_padding_in_enum_value_continuation_lines() {
    check(
        "enum Offset {\n    PAYLOAD_OFFSET =\n        PREFIX_SIZE +\n        PAYLOAD_CAPACITY * PAYLOAD_SIZE,\n    PAYLOAD_END,\n};\n",
        TEST_SAMPLE_OPTIONS,
        "enum Offset {\n    PAYLOAD_OFFSET =\n        PREFIX_SIZE +\n        PAYLOAD_CAPACITY * PAYLOAD_SIZE,\n    PAYLOAD_END,\n};\n",
    );
}

#[test]
fn pad_oper_keeps_xor_padding_in_return_continuation_lines() {
    check(
        "static uint32_t check_value(void)\n{\n    return magic ^ version ^ saved ^\n           status ^ PAYLOAD_CHECK;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static uint32_t check_value(void)\n{\n    return magic ^ version ^ saved ^\n           status ^ PAYLOAD_CHECK;\n}\n",
    );
}

#[test]
fn pad_oper_keeps_logical_padding_after_comparison_continuation_lines() {
    check(
        "static bool check_value(void)\n{\n    const bool clear =\n        decode(record, model, &candidate) ==\n        RESULT_READY && candidate.kind == KIND_CLEAR;\n    const bool ready =\n        flag_a ||\n        flag_b && item.active;\n    return clear && ready;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static bool check_value(void)\n{\n    const bool clear =\n        decode(record, model, &candidate) ==\n        RESULT_READY && candidate.kind == KIND_CLEAR;\n    const bool ready =\n        flag_a ||\n        flag_b && item.active;\n    return clear && ready;\n}\n",
    );
}

#[test]
fn attribute_declaration_does_not_leak_indent_onto_following_declaration() {
    check(
        "#define SAMPLE_OPTION 1\n\nstatic int check_value(void)\n{\n    return SAMPLE_OPTION;\n}\n\n[[noreturn]] void finish_task(void);\n\nstatic void begin_task(void);\n",
        TEST_SAMPLE_OPTIONS,
        "#define SAMPLE_OPTION 1\n\nstatic int check_value(void)\n{\n    return SAMPLE_OPTION;\n}\n\n[[noreturn]] void finish_task(void);\n\nstatic void begin_task(void);\n",
    );
}

#[test]
fn call_continuation_after_switch_keeps_assignment_indent() {
    let input = "static void handle(int kind)\n{\n    switch (kind) {\n        case 1:\n            recovery_fail();\n            return;\n        default:\n            break;\n    }\n    const enum clear_start_status clear_status =\n        stage_clear_start(\n            &app.reset_transaction,\n            ITEM_ID\n        );\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn call_argument_after_switch_keeps_statement_indent() {
    let input = "static void handle(int kind)\n{\n    switch (kind) {\n        case 1:\n            recovery_fail();\n            return;\n        default:\n            break;\n    }\n    decode(\n        first_block,\n        SOURCE_A\n    );\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn compound_literal_call_arguments_stand_a_level_past_the_statement() {
    let input = "static void check_value(void)\n{\n    CHECK(run_case(\n               request,\n               sizeof(request),\n    (struct call_case) {\n        .first_active = true,\n        .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n";
    let expected = "static void check_value(void)\n{\n    CHECK(run_case(\n        request,\n        sizeof(request),\n        (struct call_case) {\n            .first_active = true,\n            .second_busy = true,\n        },\n        &action\n          ) == RESULT_OK);\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, expected);
    check(expected, TEST_SAMPLE_OPTIONS, expected);
}

#[test]
fn statement_after_subscripted_compound_literal_uses_block_indent() {
    check(
        "#include <stddef.h>\n\nvoid test(void)\n{\n    for (size_t index = 0u; index < 3u; ++index) {\n        const uint8_t action = (uint8_t[]) {\n            FIRST,\n            SECOND,\n            THIRD,\n        }[index];\n\n          const enum status step = call(action);\n        use(step);\n    }\n}\n",
        TEST_SAMPLE_OPTIONS,
        "#include <stddef.h>\n\nvoid test(void)\n{\n    for (size_t index = 0u; index < 3u; ++index) {\n        const uint8_t action = (uint8_t[]) {\n            FIRST,\n            SECOND,\n            THIRD,\n        }[index];\n\n        const enum status step = call(action);\n        use(step);\n    }\n}\n",
    );
}

#[test]
fn compound_literal_call_arguments_reindent_from_argument_column() {
    check(
        "static void check_value(void)\n{\n    CHECK(run_case(\n               request,\n               sizeof(request),\n               (struct call_case) {\n                   .first_active = true,\n                   .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static void check_value(void)\n{\n    CHECK(run_case(\n        request,\n        sizeof(request),\n        (struct call_case) {\n            .first_active = true,\n            .second_busy = true,\n        },\n        &action\n          ) == RESULT_OK);\n}\n",
    );
}

#[test]
fn float_literal_multiplication_keeps_operator_padding() {
    check(
        "void test(void)\n{\n    const double base = a0 + alpha * db +\n                        beta * 0.1 * db2 +\n                        gamma * 0.001 * db3 +\n                        delta * 0.0001 * db4 +\n                        epsilon * 0.000001 * db5;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "void test(void)\n{\n    const double base = a0 + alpha * db +\n                        beta * 0.1 * db2 +\n                        gamma * 0.001 * db3 +\n                        delta * 0.0001 * db4 +\n                        epsilon * 0.000001 * db5;\n}\n",
    );
}

#[test]
fn nested_struct_in_union_inside_struct_keeps_closing_brace_indent() {
    check(
        "struct Item {\n    int id;\n    union {\n        struct {\n            int a;\n        } alpha;\n        struct {\n            int b;\n        } beta;\n    } value;\n};\n",
        TEST_SAMPLE_OPTIONS,
        "struct Item {\n    int id;\n    union {\n        struct {\n            int a;\n        } alpha;\n        struct {\n            int b;\n        } beta;\n    } value;\n};\n",
    );
}

#[test]
fn nested_call_on_condition_continuation_aligns_closing_paren() {
    check(
        "bool test(void)\n{\n    if (first != COMMIT ||\n        last != COMMIT ||\n        check != value_check(\n            magic,\n            status\n        )) {\n        return false;\n    }\n    return true;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "bool test(void)\n{\n    if (first != COMMIT ||\n        last != COMMIT ||\n        check != value_check(\n            magic,\n            status\n        )) {\n        return false;\n    }\n    return true;\n}\n",
    );
}

#[test]
fn multiline_call_inside_if_condition_indents_body_properly() {
    check(
        "void test(void)\n{\n    if (call(\n            payload, &settings\n        ) != RESULT_OK) {\n        finish(RESULT_INVALID);\n        return;\n    }\n}\n",
        TEST_SAMPLE_OPTIONS,
        "void test(void)\n{\n    if (call(\n            payload, &settings\n        ) != RESULT_OK) {\n        finish(RESULT_INVALID);\n        return;\n    }\n}\n",
    );
}

#[test]
fn multiline_negated_call_in_if_condition_aligns_closing_paren_and_body() {
    check(
        "static int test(uint32_t now, uint32_t started_at, uint32_t interval)\n{\n    if (!elapsed_at_least(\n            now, started_at, interval\n        )) {\n        return IDLE;\n    }\n    return BUSY;\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static int test(uint32_t now, uint32_t started_at, uint32_t interval)\n{\n    if (!elapsed_at_least(\n            now, started_at, interval\n        )) {\n        return IDLE;\n    }\n    return BUSY;\n}\n",
    );
}

#[test]
fn multiline_negated_call_in_else_if_condition_aligns_closing_paren_and_body() {
    check(
        "static void test(const struct settings *previous)\n{\n    if (previous->sequence == 0u) {\n        previous->sequence = 1u;\n    } else if (!values_equal(\n                   previous, &current\n               )) {\n        previous->sequence = next_value(\n                                 previous->sequence\n                             );\n    }\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static void test(const struct settings *previous)\n{\n    if (previous->sequence == 0u) {\n        previous->sequence = 1u;\n    } else if (!values_equal(\n                   previous, &current\n               )) {\n        previous->sequence = next_value(\n                                 previous->sequence\n                             );\n    }\n}\n",
    );
}

#[test]
fn struct_initializer_inside_switch_case_keeps_closing_brace_indent() {
    check(
        "bool test(int kind)\n{\n    switch (kind) {\n        case 1:\n            struct Info x = {\n                .field = 3u,\n                .flag = false,\n            };\n            return true;\n        default:\n            return false;\n    }\n}\n",
        TEST_SAMPLE_OPTIONS,
        "bool test(int kind)\n{\n    switch (kind) {\n        case 1:\n            struct Info x = {\n                .field = 3u,\n                .flag = false,\n            };\n            return true;\n        default:\n            return false;\n    }\n}\n",
    );
}

#[test]
fn pointer_declaration_after_case_label_keeps_name_alignment() {
    let input = "void test(int kind)\n{\n    switch (kind) {\n        case 1u:\n            Foo *value = call();\n            use(value);\n            break;\n        default:\n            break;\n    }\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn pointer_declaration_after_question_char_case_label_keeps_name_alignment() {
    let input = "void test(int kind)\n{\n    switch (kind) {\n        case '?':\n            Foo *value = call();\n            use(value);\n            break;\n        default:\n            break;\n    }\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn pointer_declaration_after_access_specifier_keeps_name_alignment() {
    let input = "class Holder {\npublic:\n    Foo *first;\n    Bar *second;\n};\n";
    check(
        input,
        TEST_SAMPLE_OPTIONS,
        "class Holder\n{\npublic:\n    Foo *first;\n    Bar *second;\n};\n",
    );
}

#[test]
fn digit_suffix_type_pointer_declaration_keeps_name_alignment() {
    let input = "void test(void)\n{\n    int64 *first = 0;\n    u32 *second = 0;\n    use(first, second);\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn typedef_pointer_with_digit_suffix_type_keeps_name_alignment() {
    let input = "typedef int64 *int64_ptr;\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn digit_suffix_word_multiplication_keeps_operator_padding() {
    let input = "void test(void)\n{\n    return base2 * scale2;\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn ternary_arm_multiplies_keep_operator_padding() {
    let input = "void test(void)\n{\n    result = ready ?\n             first * second :\n             third * fourth;\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn split_ternary_single_word_arm_multiplies_keep_operator_padding() {
    let input = "void test(void)\n{\n    result = ready ?\n             first :\n             second * third;\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn comparison_operator_continuation_multiplies_keep_operator_padding() {
    let input = "void test(void)\n{\n    value = a >\n            b * c;\n    other = a >>\n            b * c;\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn template_header_close_keeps_following_pointer_declaration() {
    let input = "template <typename T>\nT *value = nullptr;\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn ternary_constant_in_case_label_keeps_case_body_indent() {
    check(
        "void test(int kind)\n{\n    switch (kind) {\n        case 1 ? 2 : 3:\n            call();\n            break;\n        default:\n            break;\n    }\n}\n",
        TEST_SAMPLE_OPTIONS,
        "void test(int kind)\n{\n    switch (kind) {\n        case 1?2 : 3:\n            call();\n            break;\n        default:\n            break;\n    }\n}\n",
    );
}

#[test]
fn ternary_constant_in_nested_case_label_keeps_case_body_indent() {
    check(
        "void test(int kind)\n{\n    switch (kind) {\n        case 1:\n        case 2 ? 3 : 4:\n            call();\n            break;\n        default:\n            break;\n    }\n}\n",
        TEST_SAMPLE_OPTIONS,
        "void test(int kind)\n{\n    switch (kind) {\n        case 1:\n        case 2?3 : 4:\n            call();\n            break;\n        default:\n            break;\n    }\n}\n",
    );
}

#[test]
fn completed_ternary_statement_then_statement_keeps_case_body_indent() {
    let input = "void test(int kind)\n{\n    switch (kind) {\n        case 1:\n            value = ready ? first : second;\n            next();\n            break;\n        default:\n            break;\n    }\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn split_ternary_false_arm_in_case_block_keeps_continuation_column() {
    let input = "void test(int kind)\n{\n    switch (kind) {\n        case 1:\n            value = a ? b :\n                    c;\n            next();\n            break;\n        default:\n            break;\n    }\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
}

#[test]
fn else_in_alternative_branch_of_split_else_chain_keeps_chain_level() {
    let input = "void f(void)\n{\n#if defined(X)\n    if(q)\n        h = 1;\n#endif\n#ifdef R\n    if(a)\n    {\n    }\n    else\n#endif\n        if(b)\n        {\n        }\n#ifdef H\n        else\n        {\n        }\n#else\n        else\n        {\n        }\n#endif\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn statement_after_braceless_body_split_by_branches_follows_the_header() {
    let input = "void f(void)\n{\n    for (; i < n; i++)\n        if (x)\n        {\n            if (!fail)\n#ifndef W\n                a(\"%s\",\n                  s);\n#else\n                a(\"%ls\",\n                  s);\n#endif\n            b();\n            fail = 1;\n        }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn gnu_brace_after_split_else_chain_statement_follows_its_header() {
    let input = "{\n#if A\n    if(q)\n        {\n        }\n    else\n#endif\n        if(c)   /* it is missing */\n            {\n#ifndef P\n                if(cf)\n                    {\n                        result = 1;\n                    }\n                else\n#endif\n                    {\n                    }\n                if(result)\n                    {\n                        g();\n                    }\n            }\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn gnu_comment_alone_in_split_else_block_stands_at_its_body() {
    let input = "void f(void)\n{\n#ifdef A\n    if(x)\n        {\n            m = 1;\n        }\n    else\n#endif\n#ifdef B\n        if(z)\n            {\n                m = 2;\n            }\n        else\n#endif\n            {\n                /* required */\n            }\n\n    if(y)\n        g();\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn comment_before_directives_takes_the_statement_level() {
    let input = "void f(void)\n{\n    switch(c)\n        {\n        case 213:\n        {\n            if(x)\n                {\n                }\n            /* If we asked\n               we emulate */\n#if A\n#endif\n            if(y)\n                {\n                }\n        }\n        }\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn gnu_dangling_else_block_indents_its_braces() {
    let input = "void f(void)\n{\n    if(p)\n        switch(action)\n            {\n            case deny:\n                break;\n            }\n    else\n        {\n            /* If they */\n            if(action == set)\n                protoset[0] = NULL;\n            return PARAM_BAD_USE;\n        }\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn statement_after_labeled_switch_in_case_body_keeps_case_body_column() {
    let input = "void f(void)\n{\n    switch (c)\n        {\n        case '\\0':\n            i = 0;\nsegment_start:\n            switch (*path)\n                {\n                case 'a':\n                    break;\n                default:\n                    continue;\n                }\n            /*\n             * So far\n             */\n            i++;\n            break;\n        }\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn one_line_block_after_macro_and_line_directive_stays_on_its_line() {
    let input = "int f(void)\n{\n    switch (x) {\n    case 1:\n        YY_RULE_SETUP\n#line 41 \"src/lexer.l\"\n        { yy_push_state(IN_COMMENT, yyscanner); }\n        YY_BREAK\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn whitesmith_indents_one_line_block_kept_after_macro() {
    let input = "int f(void)\n    {\n    switch (x)\n        {\n        case 4:\n            YY_RULE_SETUP\n                { return NEQ; }\n            YY_BREAK\n        }\n    MAC2\n        { b(); }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn bracket_designator_row_after_one_line_row_keeps_row_column() {
    check(
        "static struct e s[] = {\n    { \"x\", \"y\" },\n    [B] = { \"x\",\n        \"y\"\n    },\n    [C] = 1,\n};\n",
        &["--style=kr"],
        "static struct e s[] = {\n    { \"x\", \"y\" },\n    [B] = {\n        \"x\",\n        \"y\"\n    },\n    [C] = 1,\n};\n",
    );
}

#[test]
fn leading_assignment_in_indented_brace_block_continues_its_target() {
    let input = "void f(void)\n{\n    if (a)\n        {\n        x[1]\n            = y;\n        }\n    z[1]\n        = y;\n}\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn horstmann_comment_ending_block_in_case_stands_at_block_body() {
    let input = "void f(void)\n{   switch (c)\n    {   case 't':\n        {   enum object_type e = t(x);\n            {   struct object_id blob_oid;\n                {   char *buffer = odb_read_object(the_repository->objects,\n                                                   &oid, &type, &size);\n                }\n                /*\n                 * we attempted to dereference a tag to a blob\n                 */\n            }\n        }\n    }\n}\n";
    check(input, &["--style=horstmann"], input);
}

#[test]
fn block_of_later_else_if_in_braceless_loop_body_keeps_its_body_level() {
    let input = "void f(void)\n    {\n    for (i = 0; i < 8; i++)\n        if (a)\n            return 0;\n        else if (b)\n            {\n            c();\n            }\n        else if (d)\n            {\n            e();\n            }\n        else\n            return 0;\n    }\n";
    check(input, &["--style=whitesmith"], input);
    let horstmann = "void f(void)\n{   for (i = 0; i < 8; i++)\n        if (a)\n            return 0;\n        else if (b)\n        {   c();\n        }\n        else if (d)\n        {   e();\n        }\n        else\n            return 0;\n}\n";
    check(horstmann, &["--style=horstmann"], horstmann);
}

#[test]
fn whitesmith_member_after_comment_following_nested_struct_keeps_member_column() {
    let input = "struct SingleRequest\n    {\n    struct\n        {\n        BIT(paused);\n        } writer;\n    /* Client Reader stack, handles\n     * checks. */\n    struct\n        {\n        struct Curl_creader *stack;\n        } reader;\n    int x;\n    };\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn vtk_file_scope_struct_array_keeps_brace_rows_at_its_rows() {
    let input = "static const struct t tests[] = {\n    { /* a */\n        \"a\",\n        TRUE\n    },\n    { /* b */\n        \"b\",\n        FALSE\n    }\n};\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn whitesmith_comment_before_else_of_split_chain_stands_at_outer_else() {
    let input = "void f(void)\n    {\n#ifdef R\n    if(a)\n        {\n        x();\n        }\n    else\n#endif\n\n        if(\n#if defined(Q) && \\\n  (Z == 1)\n            q &&\n#endif\n            b)\n            {\n            y();\n            }\n    /*\n     * Even when\n     */\n#ifdef H\n        else\n            {\n            z();\n            }\n#else\n        else\n            {\n            w();\n            }\n#endif\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn whitesmith_rows_after_run_in_nested_brace_stand_as_if_the_brace_were_broken() {
    let input = "void f(void)\n    {\n    const char *a[][2] = { { \"k1\", \"v1\" },\n            { \"k2\", \"v2\" }\n        };\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn whitesmith_label_in_case_block_keeps_comments_and_statement_after_its_switch() {
    let input = "void f(void)\n    {\n    while (true)\n        {\n        switch (*f)\n            {\n            case '%':\n                {\n                f++;\n                /* Width. */\nlabel_width:\n                switch (*f)\n                    {\n                    case '*':\n                        f++;\n                        break;\n                    default:\n                        break;\n                    }\n                /* Width/precision separator. */\n                if (*f == '.')\n                    {\n                    f++;\n                    }\n                else\n                    {\n                    goto label_length;\n                    }\n                break;\n                }\n            default:\n                break;\n            }\n        }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn trailing_comment_on_case_body_statement_keeps_it_before_next_label() {
    let input = "void *f(int n)\n{\n    switch (t)\n    {\n    case 1:    /* C closure */\n    {\n        if (n)\n            return a;\n        /* else */\n        }  /* FALLTHROUGH */\n    case 2:\n        return NULL;  /* light */\n    default:\n    {\n        g();\n        return NULL;\n    }\n    }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn vtk_knr_function_braces_stay_in_column_one() {
    let input = "char *\nre_comp (s)\nconst char *s;\n{\n    int x;\n    return 0;\n}\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn vtk_continued_first_statement_of_broken_case_block_keeps_body_column() {
    let input = "int f(const nghttp2_frame *frame, char *buffer,\n      size_t blen)\n{\n    switch(frame->hd.type)\n        {\n        case NGHTTP2_DATA:\n            {\n            return g(buffer, blen,\n                     (int)frame->data.padlen);\n            }\n        case NGHTTP2_HEADERS:\n            {\n            return 1;\n            }\n        }\n}\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn comment_before_directive_ahead_of_case_label_stays_in_case_body() {
    let input = "int f(int ch)\n    {\n    switch (x)\n        {\n        case OP_A:\n            return 1;\n#ifdef RE_ENABLE_I18N\n        case OP_UTF8_PERIOD:\n            if (ch >= 0x80)\n                return 0;\n            /* FALLTHROUGH */\n#endif\n        case OP_PERIOD:\n            return 2;\n        }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn paren_in_comment_body_line_continues_no_comment_after_it() {
    let input =
        "/* Define.\n   (The x comments, so\n   do not delete them!)  */\n/* begin syntaxes */\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn do_block_as_braceless_if_body_closes_at_do_level_after_directives() {
    let input = "void f(void)\n{\n    if (!decCheckMath(rhs, set, &status)) do { // protect allocation\n#if DECSUBSET\n            if (!set->extended) {\n                x();\n            }\n#endif\n            decExpOp(res, rhs, set, &status);\n        } while(0);                         // end protected\n    g();\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_before_directive_ahead_of_case_label_stands_past_the_label() {
    let input = "int f(int op)\n{\n    switch( op ) {\n\n        /* Mutex configuration options are only available in a threadsafe\n        ** compile.\n        */\n#if defined(SQLITE_THREADSAFE)\n    case 1: {\n        x();\n        break;\n    }\n#endif\n        /* EVIDENCE heap */\n#if defined(A)\n    case 2: {\n        y();\n        break;\n    }\n#endif\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn statements_after_directive_groups_in_braceless_do_body_keep_their_level() {
    let input = "int f(void)\n{\n    if (!decCheckMath(rhs, set, &status)) do { // protect malloc\n#if DECSUBSET\n            if (!set->extended) {\n                x();\n            } // extended=0\n#endif\n\n            decContextDefault(&aset, DEC_INIT_DECIMAL64); // clean context\n\n            if (!(rhs->bits&(DECNEG|DECSPECIAL)) && !ISZERO(rhs)) {\n                Int residue=0;               // (no residue)\n                if (!(copystat&DEC_Inexact) && w->lsu[0]==1) {\n                    // the exponent, conveniently, is the power of 10\n                    decNumberFromInt32(w, w->exponent);\n                    break;\n                } // not a power of 10\n            } // not a candidate for exact\n            decNumberZero(w);                   // set up 10...\n#if DECDPUN==1\n            w->lsu[1]=1;\n#else\n            w->lsu[0]=10;                       // ..\n#endif\n            w->digits=2;                        // ..\n        } while(0);                         // [for break]\n    return 0;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn gnu_nested_brace_in_blank_split_else_body_stands_past_its_header() {
    let input = "int f(void)\n{\n    if( a )\n        {\n            x();\n        }\n    else\n\n        /* changeset FILENAME concat */\n        if( strcmp(argv[2],\"concat\")==0 )\n            {\n                int szB;\n                if( out==0 )\n                    {\n                        exit(1);\n                    }\n                fclose(out);\n            }\n    return 0;\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn gnu_block_after_branches_ending_with_header_is_the_header_body() {
    let input = "static void f(int argc)\n{\n    int *p = 0;\n#ifdef SQLITE_ENABLE_STAT4\n    int eCall = 1;\n    if( eCall==STAT_GET_STAT1 )\n#else\n    assert( argc==1 );\n#endif\n        {\n            /* Return the value */\n            x();\n            y();\n        }\n    z();\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn whitesmith_designator_row_after_nested_close_keeps_row_column() {
    let input = "static struct patch_mode patch_mode_add =\n    {\n    .diff_cmd = { \"diff-files\", NULL },\n    .prompt_mode = {\n        N_(\"Stage mode change%s [y,n,q,a,d%s,?]? \"),\n        N_(\"Stage this hunk%s [y,n,q,a,d%s,?]? \")\n        },\n    .edit_hunk_hint = N_(\"If the patch applies cleanly, the edited hunk \"\n                         \"will immediately be marked for staging.\"),\n    .help_patch_text =\n    N_(\"y - stage this hunk\\n\"\n       \"n - do not stage this hunk\\n\")\n    };\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn later_initialized_declarators_do_not_drift() {
    let input = "static const RedisModuleEvent\nRedisModuleEvent_A =\n    {\n    REDISMODULE_EVENT_A,\n    1\n    },\nRedisModuleEvent_B =\n    {\n    REDISMODULE_EVENT_B,\n    1\n    },\nRedisModuleEvent_C =\n    {\n    REDISMODULE_EVENT_C,\n    1\n    };\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn comment_line_after_comment_text_ending_in_assignment_keeps_its_column() {
    let input = "void f(void)\n{\n    switch( x ) {\n    case A: {\n        break;\n    }\n\n    /*\n    **  PRAGMA [schema.]journal_mode =\n    **                      (delete|persist|off)\n    */\n    case B: {\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_in_empty_else_inside_case_block_keeps_body_indent() {
    let input = "void f(void)\n{\n    switch( x ) {\n    case A: {\n        if( a ) {\n        } else if( b ) {\n            /* c */\n        } else {\n            /* d */\n        }\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn struct_in_case_block_keeps_member_and_closing_columns() {
    let input = "void f(void)\n{\n    switch( x ) {\n    case A: {\n        static const struct EncName {\n            char *zName;\n            u8 enc;\n        } encnames[] = {\n            { \"UTF8\", 1 },\n            { 0, 0 }\n        };\n        x();\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn case_block_after_comment_led_label_closes_at_label() {
    let input = "void f(void)\n{\n    switch( x ) {\n    case A: {\n        break;\n    }\n    /*case B*/ default: {\n        if( z ) {\n            y();\n        }\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_after_endif_past_finished_one_line_if_keeps_statement_column() {
    let input = "int f(void)\n{\n#if (A >= 1)\n    g(x != NULL);\n#else\n    if (x == NULL) return 1;\n#endif\n\n    /* Fill */\n    {\n        size_t pos = 0;\n    }\n    return 0;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn else_block_in_braceless_loop_body_keeps_its_statements_together() {
    let input = "void f(void)\n    {\n    while (o)\n        if (a)\n            {\n            b = 1;\n            }\n        else\n            {\n            b = NULL;\n            o = NULL;\n            o = NULL;\n            }\n    x();\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn comment_after_finished_else_before_directive_keeps_statement_column() {
    let input = "void f(void)\n    {\n    if (a)\n        b = 1;\n    else\n        c |= 2;\n\n#ifndef NDEBUG\n    /* Validate */\n        {\n        DWORD mode;\n        }\n#endif\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn else_after_braced_loop_in_braceless_if_body_closes_the_body() {
    let input = "int f(void)\n{\n    if (t)\n        while (a)\n        {\n            b();\n        }\n    else\n        /* other */\n        while (c)\n        {\n            d();\n        }\n    return 0;\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn closing_brace_after_braces_opened_in_both_directive_branches_finds_its_opener() {
    let input = "void f(void)\n{\n    for (;;)\n    {\n        for (;;)\n        {\n#ifdef A\n            if (a)\n            {\n#else\n            if (b)\n            {\n#endif\n                x();\n            }\n        }\n    }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn designator_rows_after_run_in_first_element_align_with_it() {
    let input = "void f(void)\n{\n    struct s v = { .a = 1,\n                   .b = 2\n                 };\n    x();\n}\n";
    // A struct declaration closes its aggregate at the statement.
    let expected = "void f(void)\n{\n    struct s v = { .a = 1,\n                   .b = 2\n    };\n    x();\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn statement_after_labeled_block_with_directive_keeps_its_level() {
    let input = "void *f(void)\n{\n    x();\n    return o;\n\nfail:\n    if(o)\n        {\n#ifndef A\n            g(o);\n#endif\n            h(o);\n        }\n\n    LEAVE(&guard);\n    return NULL;\n}\n";
    check(input, &["--style=gnu"], input);
}

#[test]
fn ratliff_block_brace_after_directive_stays_off_the_directive() {
    let input = "int f(void) {\n    for (i = 0; i < n; i++) {\n        if (user) {\n            continue;\n            }\n\n#if !(NGX_WIN32)\n            {\n            int fi;\n            g(fi);\n            }\n#endif\n        }\n    return 0;\n    }\n";
    check(input, &["--style=ratliff"], input);
}

#[test]
fn ratliff_function_brace_after_endif_indents_with_its_body() {
    let input = "#ifdef A\nint\ng(int a, int b)\n#else\nint\ng(int a)\n#endif\n    {\n    return h(a);\n    }\n\nint\nk(int a) {\n    return 0;\n    }\n";
    check(input, &["--style=ratliff"], input);
}

#[test]
fn horstmann_braceless_do_while_in_case_block_closes_at_the_do() {
    let input = "void f(void)\n{   switch (x)\n    {   case 2:\n        {   /* Read */\n            for (;;)\n            {   do\n                    r = recv(s);\n                while (r == -1);\n\n                if (r <= 0)\n                    break;\n            }\n            break;\n        }\n    }\n}\n";
    check(input, &["--style=horstmann"], input);
}

#[test]
fn whitesmith_braceless_chain_block_in_case_body_keeps_one_level() {
    let input = "int f(void)\n    {\n    switch (x)\n        {\n        case 2:\n            if (a)\n                return 1;\n            else\n                {\n                ret = g();\n\n                if (ret == 0)\n                    for (i = 0; i < 2; i++)\n                        {\n                        h(i);\n                        k(i);\n                        }\n                }\n\n            return ret;\n        }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn whitesmith_do_block_after_commented_braceless_if_in_case_keeps_its_body() {
    let input = "void f(void)\n    {\n    switch (x)\n        {\n        case 1:\n            if (put < end)\n                {\n                if (num)\n                    // Insert\n                    do\n                        {\n                        buf += 1;\n                        putc(buf, out);\n                        }\n                    while (put < end);\n                else\n                    {\n                    put = end;\n                    }\n                }\n        }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn designator_row_after_closed_nested_initializer_keeps_its_level() {
    let input = "static const T e = {\n    .db = {\n        .dbh = 0\n    },\n    .dx = 0,\n    .delim = {\n        .d = 1\n    },\n    .x = 1\n};\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn enumerator_value_after_comment_continues_past_its_assignment() {
    let input = "enum e {\n    A = 1,\n    /* c */\n    B    = 0\n           | C\n           | D,\n    E = 1\n};\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn commented_case_block_closer_lines_up_with_its_label() {
    let input = "void f(void)\n{\n    switch (r)\n    {\n    case 1:\n    {\n        x();\n        break;\n    } // r-d\n    case 2:\n    {\n        break;\n    } // r-h-d\n    }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn else_after_semicolonless_macro_body_starts_its_own_line() {
    let input = "static void\nf(char *a, int swaptype)\n{\n\n    if (swaptype <= 1)\n        swapcode(long, a, b, n)\n    else\n        swapcode(char, a, b, n)\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_after_block_in_case_body_stands_at_the_block_header() {
    let input = "void f(void)\n{\n    switch(t)\n        {\n        case A:\n            if (!z)\n                {\n                r();\n                return NULL;\n                }\n            /* Convert to ziplist encoded hash. This must be deprecated\n             * when loading dumps. */\n                {\n                unsigned char *lp = lpNew(0);\n                }\n            break;\n        }\n}\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn commented_switch_closer_after_else_switch_keeps_the_body_level() {
    let input = "void f(void)\n{\n    if (a) {\n        b = 1;\n    } else switch (r) {\n        case 1: {\n            break;\n        } // r-d\n        default: {\n            break;\n        }\n        } // switch\n    x();\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn vtk_block_after_split_macro_loop_head_stands_past_its_start() {
    let input = "void f(void)\n{\n    x();\n    RB_FOREACH_SAFE(watcher_list, watcher_root,\n                    uv__inotify_watchers(loop), tmp_watcher_list_iter)\n        {\n        watcher_list->iterating = 1;\n        y();\n        }\n}\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn whitesmith_statements_before_the_first_label_stand_in_the_case_body() {
    let input = "void f(void)\n    {\n    switch( iSub )\n        {\n            CASE(0, \"x\")\n                {\n                int nCol;\n                nCol = g();\n                break;\n                }\n            /* Next */\n            CASE(1, \"y\")\n                {\n                int nRow;\n                break;\n                }\n        }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn last_line_of_a_case_label_split_over_lines_continues_it() {
    let input = "void f(void)\n{\n    switch (flags & (A |\n                     B)) {\n    case A |\n            B |\n            C:\n        set = 1;\n        break;\n    case A:\n        break;\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_after_continued_define_ignores_its_parens() {
    let input = "#define SYN\t\t\t\t\\\n  (_A  | B\t\t\t\\\n   | C)\n/* [[[end syntaxes]]] */\n\n\n/* Maximum number of duplicates.  Some\n   systems.  */\n# ifdef RE_DUP_MAX\n#  undef RE_DUP_MAX\n# endif\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn tab_after_return_still_sets_the_value_column() {
    let input =
        "int g(void)\n{\n    return\t(uint16_t)p[0] << 8 |\n            (uint16_t)p[1] << 0;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn block_comment_before_first_initializer_element_stays_with_it() {
    let input = "static const u8 statemap[8] =\n{   /* 0 INVALID */ 1,\n    /* 1 START   */ 0,\n    /* 2 NORMAL  */ 1,\n};\n";
    check(input, &["--style=horstmann"], input);
}

#[test]
fn macro_word_heading_a_block_keeps_the_block() {
    let input = "int f(Wal *pWal)\n{   int rc;\n    SEH_TRY\n    {   rc = g(pWal);\n    }\n    SEH_EXCEPT( rc = 1; )\n    return rc;\n}\n";
    check(input, &["--style=horstmann"], input);
}

#[test]
fn macro_word_heading_a_block_after_a_comment_keeps_the_block() {
    let input = "void f(void)\n{   x();\n\n    /* c */\n    SEH_TRY\n    {   if( rc==SQLITE_OK )\n        {   g();\n        }\n    }\n}\n";
    check(input, &["--style=horstmann"], input);
}

#[test]
fn ratliff_initializer_after_struct_close_nests_from_the_declaration() {
    let input = "static const struct {\n    const char *key;\n    int level;\n    } advice_setting[] = {\n    [A]\t= { \"a\" },\n    [B]\t= { \"b\" },\n    };\nstatic const struct {\n    const char *string;\n    int key;\n    } key_string_table[] = {\n    /* Function keys. */\n        { \"F1\", 1 },\n        { \"F2\", 2 },\n\n    /* Arrow keys. */\n        { \"Up\", 3 },\n    };\n";
    check(input, &["--style=ratliff"], input);
}

#[test]
fn nested_compound_literal_brace_rows_follow_the_style() {
    let ratliff = "static T a[] = {\n        {\n        .x = 1\n        },\n        {0}\n    };\nvoid f(void) {\n    g(&(T) {\n        .k = (S[]) {\n                {\n                .x = 1\n                },\n                {0}\n            },\n        });\n    }\n";
    check(ratliff, &["--style=ratliff"], ratliff);
    let whitesmith = "static T a[] = {\n        {\n        .x = 1\n        },\n        {0}\n    };\nvoid f(void)\n    {\n    g(&(T)\n        {\n        .k = (S[])\n            {\n                {\n                .x = 1\n                },\n                {0}\n            },\n        });\n    }\n";
    check(whitesmith, &["--style=whitesmith"], whitesmith);
}

#[test]
fn ratliff_rows_after_a_split_brace_row_close_at_the_row() {
    let input = "void f(void) {\n    T info = {\n        .k = (S[]) {\n                {\n                .a = 1,\n                .b = {0,1,0}\n                }, {\n                .c = 2,\n                /* Omitted is RANGE {0,1,0} */\n                },\n                {0}\n            }\n        };\n    }\n";
    check(input, &["--style=ratliff"], input);
}

#[test]
fn vtk_comment_before_indented_brace_row_stays_at_the_rows() {
    let input = "static const T st[][16] =\n{\n    /* 0 */\n        {\n            {0x00, 0x03}, {0x01, 0x04}\n        },\n    /* 5 */\n        {\n            {0x03, 0x01}, {0x06, 0x01}\n        }\n};\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn whitesmith_class_with_base_list_on_its_line_indents_its_brace() {
    let input = "class gzfilebuf : public streambuf\n    {\n\n    public:\n\n        gzfilebuf( );\n        int x;\n    };\n\nclass A\n    {\n    public:\n        int y;\n    };\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn whitesmith_macro_loop_after_a_statement_indents_its_brace() {
    let input = "void f(void)\n    {\n    if (flags)\n        {\n        spans = g();\n        TAILQ_FOREACH(span, spans, entry)\n            {\n            if (span)\n                h(span);\n            }\n        }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn comment_after_file_scope_macro_call_starts_at_column_one() {
    let input = "CTL_RO_CGEN(config_stats, a,\n            b, uint64_t)\n/*\n * Note.\n */\nCTL_RO_CGEN(config_stats, c,\n            d, uint64_t)\n\n/* Lock profiling related APIs below. */\n#define X 1\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn whitesmith_compound_literal_brace_in_nested_block_follows_its_statement() {
    let input = "void f(void)\n    {\n    if (x)\n        {\n        *promise = (P)\n            {\n            .ref_count = 2,\n            .c = c,\n            };\n        }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn source_attached_nested_initializer_brace_keeps_its_statement_indent() {
    let input = "void f(void)\n{\n    int y;\n    struct r r1[] = { {\n            .a = 1,\n        }\n    };\n}\n";
    check(input, &["--style=allman"], input);
    check(input, &["--style=gnu"], input);
}

#[test]
fn struct_with_broken_brace_in_case_block_indents_members_once() {
    let input = "void f(void)\n{\n    switch (op)\n    {\n    case A:\n    {\n        struct\n        {\n            int m;\n        } k;\n    }\n    }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn ratliff_braced_do_as_braceless_body_puts_while_at_the_body() {
    let input = "void f(void) {\n    if (left) do {\n            g();\n            }\n        while (left);\n    h();\n    }\n";
    check(input, &["--style=ratliff"], input);
}

#[test]
fn ratliff_case_block_closed_with_semicolon_stays_at_its_body() {
    let input = "void f(void) {\n    switch (x) {\n        case A: {\n            g();\n            break;\n            };\n\n        case B:\n            break;\n        }\n    }\n";
    check(input, &["--style=ratliff"], input);
}

#[test]
fn ratliff_statements_in_a_braced_chain_of_a_braceless_body_keep_their_block() {
    let input = "void f(void) {\n    for (;;)\n        if (a)\n            b = 0;\n        else {\n            if (h) {\n                c = h;\n                rc++;\n                }\n            }\n    return rc;\n    }\n";
    check(input, &["--style=ratliff"], input);
}

#[test]
fn vtk_comment_ending_a_file_scope_array_stays_at_its_rows() {
    let input = "const unsigned char sane_ctype[256] =\n{\n    A, X,\t\t/* 112..127 */\n    /* Nothing in the 128.. range */\n};\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn vtk_compound_literal_after_a_dereferenced_assignment_indents_its_brace() {
    let input = "void f(void)\n{\n    *r = (A)\n        {\n        .name = name,\n        .desc = desc,\n        };\n}\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn whitesmith_blocks_in_an_else_body_after_a_blank_line_follow_their_headers() {
    let input = "void f(void)\n    {\n    if( a )\n        b();\n    else\n\n#ifndef X\n        if( c )\n            {\n            for(;;)\n                {\n                g();\n                }\n            }\n#endif\n    }\n";
    check(input, &["--style=whitesmith"], input);
    check(
        input,
        &["--style=vtk"],
        &input
            .replacen("    {\n    if", "{\n    if", 1)
            .replacen("#endif\n    }", "#endif\n}", 1),
    );
}

#[test]
fn whitesmith_case_block_in_an_else_body_after_a_blank_line_keeps_its_statements() {
    let input = "void f(void)\n    {\n    if( a )\n        b();\n    else\n\n        if( c )\n            {\n            switch( op )\n                {\n                case X:\n                    {\n                    g();\n                    for(;;)\n                        {\n                        h();\n                        }\n                    break;\n                    }\n                }\n            }\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn macro_loop_brace_after_a_case_label_is_no_case_block() {
    let input = "void f(void)\n{\n    switch (x)\n        {\n        case A:\n            RB_FOREACH_SAFE(field, json_fields, &node->fields, field1)\n            {\n                g(field);\n            }\n            break;\n        }\n}\n";
    check(input, &["--style=gnu"], input);
    let input = "void f(void)\n{\n    switch (x)\n        {\n        case A:\n            RB_FOREACH_SAFE(field, json_fields, &node->fields, field1)\n                {\n                g(field);\n                }\n            break;\n        }\n}\n";
    check(input, &["--style=vtk"], input);
}

#[test]
fn brace_after_a_struct_declarator_in_a_case_block_keeps_the_body() {
    let input = "void f(void)\n{\n    switch (x)\n    {\n    case 11:\n    {\n        struct O\n        {\n            int opt;\n        } aOpt[] =\n        {\n            { 0, 0 }\n        };\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn condition_continuation_in_a_case_body_follows_astyles_stack() {
    let input = "void f(void)\n{\n    switch (x)\n    {\n    case 1:\n        if (ngx_quic_handle(c, pkt,\n                            &frame)\n                != NGX_OK)\n        {\n            return NGX_ERROR;\n        }\n    }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn condition_continuation_after_sizeof_struct_follows_astyles_stack() {
    let input = "void f(void)\n{\n    x();\n    if (setsockopt(fd,\n                   &af, sizeof(struct accept_filter_arg))\n            == -1)\n    {\n    }\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn ternary_arms_in_a_block_after_a_directive_follow_their_parens() {
    let input = "void f(void)\n{\n# ifdef X\n    {\n        wchar_t wc;\n\n        start_ch = ((start_elem->type == SB_CHAR) ? start_elem->opr.ch\n                    : ((start_elem->type == COLL_SYM) ? start_elem->opr.name[0]\n                       : 0));\n    }\n# endif\n}\n";
    check(input, &["--style=allman"], input);
}

#[test]
fn bracket_operand_after_a_typedef_struct_keeps_its_bracket_column() {
    let input = "typedef struct {\n    int x;\n} a_t;\ntypedef struct {\n    u_char key[LEN\n               - sizeof(k)];\n} b_t;\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn later_sibling_rows_in_a_compound_literal_stay_with_the_first() {
    let input = "void f(void)\n{\n    T info = {\n        .version = 1,\n        .key_specs = (K[])\n        {\n            {\n                .flags = 1,\n            }, {\n                .flags = 2,\n                .x = 2,\n            }, {\n                .flags = 3,\n            },\n            {0}\n        }\n    };\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn assigned_value_split_by_directives_keeps_its_continuation() {
    let input = "void f(void)\n{\n    switch(x) {\n    case 1:\n        *p =\n#ifdef A\n            0\n#else\n            y\n#endif\n            ;\n        break;\n    }\n    *p =\n#ifdef A\n        0\n#else\n        y\n#endif\n        ;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_after_a_switch_brace_before_a_statement_stands_at_the_body() {
    let input = "void f(void)\n{\n    switch (code) {\n        /* These mappings are the same. */\n        VK_CASE(VK_INSERT,  \"[2~\")\n    default:\n        return NULL;\n    }\n    switch (x) {\n    /* first */\n    case 1:\n        break;\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn whitesmith_function_after_a_typedef_and_a_block_comment_starts_the_line() {
    let input = "typedef struct\n    {\n    int state;\n    } set_rand_t;\n/* note\n   more. */\nstatic void set_seed(int seed)\n    {\n    g = 1;\n    h = 2;\n    }\n";
    check(input, &["--style=whitesmith"], input);
}

#[test]
fn padded_commas_keep_a_double_pointer_declarator_attached() {
    let input = "void f(void)\n{\n    struct C *tail, **pnext;\n    int a, **b;\n}\n";
    check(input, &["--style=1tbs", "--pad-comma"], input);
}

#[test]
fn padded_operators_keep_the_spacing_of_an_ampersand_after_a_paren() {
    let input = "void f()\n{\n    g((LPVOID)&vals);\n    x = (a) &b;\n    x = (a) & (b);\n    x = (a) & 0xff;\n}\n";
    check(input, &["--style=1tbs", "--pad-oper"], input);
}

#[test]
fn label_address_operands_stay_attached() {
    let input = "static const void *const disptab[] = {\n    &&L_OP_MOVE,\n    &&L_OP_LOADI,\n};\nvoid f()\n{\n    goto *&&L_X;\n    p = &&L_A;\n    y = a && b;\n}\n";
    check(input, &["--style=1tbs", "--pad-oper"], input);
}

#[test]
fn added_braces_leave_an_empty_statement_on_its_own_line() {
    let input = "void f(void)\n{\n    for (last = list; last->next; last = last->next)\n        ;\n    if (uc)\n        ; /* nothing */\n    else {\n        x = 1;\n    }\n}\n";
    check(input, &["--style=1tbs"], input);
}

#[test]
fn double_pointer_declared_in_a_for_header_keeps_its_stars_together() {
    let input = "void f(void)\n{\n    for (const char **argp = argv; *argp; argp++) {\n        x();\n    }\n}\n";
    check(
        input,
        &["--style=1tbs", "--pad-oper", "--align-pointer=name"],
        input,
    );
    check(
        input,
        &["--style=1tbs", "--pad-oper", "--align-pointer=middle"],
        &input.replace("char **argp", "char ** argp"),
    );
}

#[test]
fn added_braces_keep_a_switch_body_on_its_own_line() {
    let input = "void f(void)\n{\n    if (!tracking.matches)\n        switch (track) {\n        case 1:\n            goto cleanup;\n        }\n}\n";
    check(input, &["--style=1tbs"], input);
}

#[test]
fn indented_define_keeps_a_trailing_comment_continuation() {
    let input = "#define MAX_PROTOS 34\n#define MAX_PROTOSTRING (MAX_PROTOS * 11)  /* Room for MAX_PROTOS number of\n                                              10-chars proto names. */\n\nint x;\n";
    check(input, &["--indent-preproc-define"], input);
}

#[test]
fn added_braces_leave_a_directive_after_a_case_label_alone() {
    let input = "void f(void)\n{\n    if(!result)\n        switch(progress) {\n        case SASL_IDLE:\n#ifndef X\n            if(a)\n                /* APOP */\n            {\n                r = 1;\n            }\n#endif\n            break;\n        }\n}\n";
    check(input, &["--style=1tbs"], input);
}

#[test]
fn padded_operators_keep_unary_signs_and_derefs_attached() {
    let input = "void f(void)\n{\n    struct commit_extra_header *new;\n    remote_dir_exists[*parent] = 1;\n    if (*types == (void *) -1) {\n        return;\n    }\n    for (; cf && q; cf = cf->next) {\n    }\n    return g(data,\n             (q & CURL_DNSQ_ADDR), t);\n}\n";
    check(
        input,
        &[
            "--pad-oper",
            "--align-pointer=name",
            "--align-reference=name",
        ],
        input,
    );
}

#[test]
fn max_code_length_keeps_initializer_rows_whole() {
    let input = "static const int t[] = {\n    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,\n};\n";
    check(input, &["--max-code-length=80"], input);
}

#[test]
fn indented_comment_in_a_block_under_an_include_guard_keeps_the_body() {
    let input = "#ifndef BYTECODE_H\n#define BYTECODE_H\nstruct o {\n    int flags;\n\n    // length\n    int length;\n};\n#endif\n";
    check(input, &["--indent-col1-comments"], input);
}

#[test]
fn indented_define_bodies_keep_braceless_bodies_and_header_parens() {
    let input = "#define for_each(it, queue) \\\n    for (size_t pq_ix_ = (queue)->get_pending; \\\n         pq_ix_ < (queue)->nr_; \\\n         pq_ix_++)\n\n#define C()  \\\n    if(s < 0) \\\n        s = in\n\nint x;\n";
    check(
        input,
        &[
            "--style=1tbs",
            "--indent-preproc-define",
            "--pad-header",
            "--min-conditional-indent=0",
        ],
        input,
    );
}

#[test]
fn added_braces_keep_empty_bodies_and_comments_after_split_conditions() {
    let input = "int f(void)\n{\n    if (c)\n        for (v = p; *v == 32; v++)\n            ;\n    if(!data->set.low_speed_time || !data->set.low_speed_limit ||\n            paused(data))\n        /* A paused transfer is not qualified */\n    {\n        return CURLE_OK;\n    }\n    return 1;\n}\n";
    check(input, &["--style=1tbs"], input);
}

#[test]
fn lisp_keeps_labels_with_statements_and_brace_literal_case_labels() {
    let input = "void g(char c) {\n    switch (c) {\n    case '{': d = 1; break; } }\nvoid h(void) {\n    x();\nnext: z(); }\n";
    check(input, &["--style=lisp"], input);
}

#[test]
fn initializer_row_after_a_split_call_row_starts_at_the_row() {
    let input = "int f(void)\n{\n    struct option opts[] = {\n        OPT_STRING(0, \"prefix\", &tree_prefix, N_(\"<prefix>/\"),\n                   N_(\"write tree object\")),\n        {\n            .type = OPTION_BIT,\n            .long_name = \"ignore\",\n        },\n        OPT_END()\n    };\n}\n";
    check(input, &["--min-conditional-indent=0"], input);
}

#[test]
fn added_braces_keep_a_multi_line_body_after_else_if_at_its_level() {
    let input = "void f(void)\n{\n    if (a) {\n        if (r)\n            die_errno(_(\"renaming\"),\n                      fname);\n    } else if (b)\n        die(_(\"pack-objects\"),\n            name);\n    else if (c) {\n        y();\n    }\n}\n";
    check(input, &["--style=1tbs"], input);
}

#[test]
fn added_braces_leave_a_statement_after_a_macro_body_alone() {
    let input =
        "void f(void)\n{\n    if( i>iLt ) SWAP_DOUBLE(a[i],a[iLt])\n        iLt++;\n    i++;\n}\n";
    check(input, &["--style=1tbs"], input);
}

#[test]
fn continuation_of_a_body_run_in_after_its_header_goes_a_level_in() {
    let input = "int f(void)\n{\n    if (arg_len % 4) {\n        if (err) *err = \"Wrong number of arguments in \"\n                            \"buffer limit configuration.\";\n        return 0;\n    }\n    if (c) addReplyError(c,\"Invalid stream ID specified as stream \"\n                             \"command argument\");\n    if (vecSize(vdeleted)) notify(NOTIFY_HASH, \"hdel\",\n                                      keyArg, c->db->id);\n    return 1;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_led_fallthrough_macro_before_a_case_label_keeps_the_body() {
    let input = "void f(int a)\n{\n    switch (a) {\n    case 1:\n        op = 2;\n        /* no break */ deliberate_fall_through\n    case 2:\n        break;\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn run_in_initializer_in_a_case_block_keeps_its_brace_column() {
    let input = "void f(int op)\n{\n    switch (op) {\n    case 1: {\n        static const char *az[] = { \"SHARED\", \"RESERVED\",\n                                    \"PENDING\", \"EXCLUSIVE\"\n                                  };\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn two_comments_on_a_line_before_a_case_label_keep_the_body_indent() {
    let input = "void f(int a)\n{\n    switch (a) {\n    case 1:\n        x();\n        /* else */ /* FALLTHROUGH */\n    case 2:\n        break;\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn initializer_rows_align_with_the_first_element_past_extra_spaces() {
    let input = "void f(void)\n{\n    static const char *ones[] = {  \"zero\", \"one\",\n                                   \"six\", \"seven\"\n                                };\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn designator_rows_in_a_case_block_indent_past_the_declaration() {
    let input = "void f(int c)\n{\n    switch (c) {\n    case 1: {\n        const struct options opts = {\n            .prefix = \"\",\n            .suffix = \"\",\n        };\n        g(&opts);\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn comment_before_the_first_compound_literal_element_takes_its_indent() {
    let input = "void f(void)\n{\n    *p = (T) {\n        /* c */\n        .a = 2,\n    };\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn star_after_a_cast_inside_parens_keeps_its_source_spacing() {
    let input =
        "void f(void)\n{\n    g((int) * cp);\n    g((int)*cp);\n    g(((double)b) * c);\n}\n";
    check(input, &["--pad-oper"], input);
}

#[test]
fn function_pointer_parameter_after_a_builtin_type_keeps_the_star_attached() {
    let input = "int a(char *(*func)(char *input), int x);\nint c(struct x * (*h)(int), int x);\n";
    check(input, &["--pad-oper", "--align-pointer=name"], input);
    let input = "int a(char* (*func)(char* input), int x);\nint c(struct x * (*h)(int), int x);\n";
    check(input, &["--pad-oper", "--align-pointer=type"], input);
}

#[test]
fn triple_pointer_cast_stays_one_run() {
    let input = "void f(void)\n{\n    n = g(e, (const char ***)&argv);\n}\n";
    check(input, &["--pad-oper", "--align-pointer=name"], input);
}

#[test]
fn multiplication_on_a_continued_argument_line_stays_padded() {
    let input = "int f(void)\n{\n    return m->num + get(m->chunk +\n                        (off_t)pos * WIDTH);\n}\n";
    check(input, &["--align-pointer=name"], input);
}

#[test]
fn define_initializer_closer_after_a_last_designator_row_keeps_its_level() {
    let input = "#define ATOM_VALUE_INIT { \\\n        .s_size = ATOM_SIZE_UNSPECIFIED \\\n    }\n";
    check(input, &["--indent-preproc-define"], input);
}

#[test]
fn else_after_a_braceless_body_ignores_an_earlier_closed_if_block() {
    let input = "void f(void)\n{\n    for (;;)\n        if (a) {\n            b = 0;\n        }\n    if (added)\n        p(1,\n          2);\n    else {\n        if (deleted)\n            printf(\"%s%sdeleted file \",\n                   line_prefix, c_meta);\n        x();\n    }\n}\n";
    check(input, &["--max-code-length=109"], input);
}

#[test]
fn define_header_body_after_a_split_condition_takes_the_body_level() {
    let input = "#define CHECK(x, y)                \\\n    do {                           \\\n        if(result &&               \\\n           result != OTHER)        \\\n            goto error;            \\\n    } while(0)\n";
    check(
        input,
        &["--indent-preproc-define", "--min-conditional-indent=0"],
        input,
    );
}

#[test]
fn member_declarator_rows_after_a_commented_row_keep_its_column() {
    let input = "struct acttab {\n    int nAction;\n    struct lookahead_action\n        *aAction,                  /* The table */\n        *aLookahead;               /* A set */\n    int mnLookahead;\n};\nstruct s {\n    robj *a,\n         *b, /* x */\n         *c;\n};\n";
    check(input, &["--style=linux"], input);
}

#[test]
fn chained_assignment_rows_align_with_the_last_assigned_value() {
    let input = "void f(void)\n{\n    *l1 = *l2 = (v) |\n                (v << 16) |\n                (v << 32);\n}\n";
    check(input, &["--style=linux"], input);
}

#[test]
fn star_after_a_numeric_cast_on_a_continued_argument_line_keeps_its_spacing() {
    let input = "void f(void)\n{\n    logmsg(\"%lu > %lu\",\n           (unsigned long)*qlen, (unsigned long)qbuflen);\n}\n";
    check(input, &["--pad-oper"], input);
}

#[test]
fn fill_empty_lines_leaves_file_scope_lines_empty() {
    let input = "/*\n * x\n */\n\n#include <a.h>\n\nvoid f(void)\n{\n    g(cf, 0,\n      name);\n      \n    /*\n     * c\n     */\n    \n    x();\n}\n";
    check(input, &["--style=kr", "--fill-empty-lines"], input);
}

#[test]
fn fill_empty_lines_restarts_each_conditional_branch() {
    let input = "void f(void)\n{\n#if A\n    if (a) {\n#else\n    if (b) {\n#endif\n        x();\n    }\n#if B\n    y();\n#endif\n    \n    z();\n}\n";
    check(input, &["--style=kr", "--fill-empty-lines"], input);
}

#[test]
fn star_after_a_comma_in_a_condition_call_stays_unary() {
    let input =
        "int f(void)\n{\n    if (icase && a(wc, *c))\n        return 1;\n    return 0;\n}\n";
    check(input, &["--style=kr", "--align-pointer=middle"], input);
}

#[test]
fn remove_braces_keeps_the_braces_of_a_statement_spanning_lines() {
    let input = "void f(void)\n{\n    if (b) {\n        rc = g(interp,\n               objv[1]);\n    }\n}\n";
    check(input, &["--style=kr", "--remove-braces"], input);
}

#[test]
fn indent_after_parens_stacks_a_level_per_open_paren_and_assignment() {
    let input = "int f(void)\n{\n    return test(\n            a,\n            b);\n    test(\n        a,\n        b);\n    if (a && b &&\n        c)\n        y();\n    if (a && ((b && (c)) ||\n            d))\n        y();\n    if (log) g(f, 0,\n            h(i));\n}\n";
    check(input, &["--style=kr", "--indent-after-parens"], input);
}

#[test]
fn indent_after_parens_indents_rows_of_a_run_in_initializer() {
    let input = "void f(void)\n{\n    static const int cat[] = {LC_ALL, LC_COLLATE, LC_CTYPE,\n            LC_NUMERIC, LC_TIME\n        };\n}\n";
    check(input, &["--style=kr", "--indent-after-parens"], input);
}

#[test]
fn indent_after_parens_leaves_ternary_arms_and_operator_rows_at_the_value_level() {
    let input = "void f(void)\n{\n    struct Curl_cfilter *cf =\n        (data->conn && V(sockindex)) ?\n        data->conn->cfilter[sockindex] : NULL;\n    lu_mem sz = cast(lu_mem, sizeof(Proto))\n        + cast_uint(p->sizep) * sizeof(Proto*)\n        + cast_uint(p->sizek);\n    if( a\n        || b\n    ) {\n        x();\n    }\n}\n";
    check(input, &["--style=kr", "--indent-after-parens"], input);
}

#[test]
fn indent_preproc_cond_opens_a_nested_conditional_at_the_code_level() {
    let input = "#ifndef G\n#define G\n\nstruct s {\n    int a;\n\n    #if (X)\n    int b;\n    #endif\n};\n\n#endif\n";
    check(input, &["--style=linux", "--indent-preproc-cond"], input);
}

#[test]
fn commented_brace_ending_a_broken_else_if_ends_the_chain() {
    let input = "void f(void)\n{\n    if (a)\n        st.st_mode = 0;\n    else\n        if (lstat(path, &st) < 0)\n        {\n            st.st_mode = 0;\n        } /* else stat is valid */\n\n    if (!verify_path(path))\n    {\n        return;\n    }\n}\n";
    check(input, &["--style=allman", "--break-elseifs"], input);
}

#[test]
fn one_line_control_block_on_its_own_line_follows_astyle_per_style() {
    let ratliff = "void f(void) {\n    if (s == NULL)\n        { return path; }\n    x();\n    }\n";
    check(
        ratliff,
        &["--style=ratliff", "--keep-one-line-blocks"],
        ratliff,
    );
    let vtk = "void f(void)\n{\n    if (s == NULL)\n    { return path; }\n    x();\n}\n";
    check(vtk, &["--style=vtk", "--keep-one-line-blocks"], vtk);
}

#[test]
fn break_one_line_headers_keeps_a_one_line_block_on_its_own_line() {
    let input = "void f(void) {\n    if (x)\n    { return 1; }\n    if (y) {\n        return 2;\n    }\n}\n";
    check(
        input,
        &[
            "--style=java",
            "--keep-one-line-blocks",
            "--break-one-line-headers",
        ],
        input,
    );
}

#[test]
fn remove_comment_prefix_leaves_rows_of_a_trailing_comment_in_place() {
    let input = "struct s {\n    unsigned int action;  /* CURL_POLL_IN we last told the\n                             libcurl application */\n    int x;      /* first\n                   second */\n};\n";
    check(input, &["--style=kr", "--remove-comment-prefix"], input);
}

#[test]
fn break_blocks_keeps_a_multi_line_comment_on_its_header() {
    let input = "void f(void)\n{\n    z();\n\n    /* two\n       lines */\n    if (b) {\n        y();\n    }\n\n    w();\n\n    /*\n     * three\n     */\n    if (c) {\n        y();\n    }\n}\n";
    check(input, &["--style=kr", "--break-blocks"], input);
}

#[test]
fn indent_preproc_block_indents_a_body_after_a_comment() {
    let input = "/* a */\n#ifndef C\n    typedef BOOL _Bool;\n#endif\n";
    check(input, &["--style=google", "--indent-preproc-block"], input);
}

#[test]
fn break_return_type_leaves_heads_led_by_struct_or_union() {
    let input = "struct notes_tree **load(struct string_list *refs, int flags)\n{\n    x();\n}\nunion u f(void)\n{\n    x();\n}\nconst struct a *\ng(void)\n{\n    x();\n}\n";
    check(input, &["--style=kr", "--break-return-type"], input);
}

#[test]
fn attach_return_type_aligns_parameters_of_a_struct_head_left_split() {
    let input = "struct style *\nstyle_add(struct grid_cell *gc, struct options *oo, const char *name,\n          struct format_tree *ft)\n{\n    x();\n}\n";
    check(input, &["--style=gnu", "--attach-return-type"], input);
}

#[test]
fn define_body_row_led_by_assignment_takes_a_continuation_level() {
    let input = "#define C(x) na B(v) \\\n        = c\n";
    check(input, &["--indent-preproc-define"], input);
}

#[test]
fn define_rows_align_with_the_first_character_after_a_paren() {
    let input = "#define DIFF_PAIR_BROKEN(p) \\\n    ( (!DIFF_FILE_VALID((p)->one) != !DIFF_FILE_VALID((p)->two)) && \\\n      ((p)->broken_pair != 0) )\n";
    check(input, &["--indent-preproc-define"], input);
}

#[test]
fn comment_led_argument_row_under_a_code_length_limit_aligns_with_the_arguments() {
    let input = "void f(void)\n{\n    x = g(a,\n          /* c */ b, /* d */ e);\n}\n";
    check(input, &["--style=kr", "--max-code-length=109"], input);
}

#[test]
fn parameters_after_an_unattached_return_type_align_with_their_paren() {
    let input = "JEMALLOC_FORMAT_PRINTF(3, 4)\nstatic void\nprof_dump_printf(write_cb_t *prof_dump_write, void *cbopaque,\n                 const char *format, ...)\n{\n    va_list ap;\n}\n";
    check(input, &["--style=kr", "--attach-return-type"], input);
}

#[test]
fn pad_oper_keeps_dereference_after_a_control_header_and_cast_stars() {
    let input = "void f(void)\n{\n    if (c == 1)\n        *out++ = 1;\n    pU8 = (u8*)pAllocation;\n    *(char**)pArg = 0;\n    v = luaM_reallocvector(L, tb->hash, osize, nsize, TString*);\n}\n";
    check(input, &["--style=kr", "--pad-oper"], input);
}

#[test]
fn unary_star_after_a_comma_or_spaced_operator_stays_attached() {
    let input = "void f(void)\n{\n    while (isspace(cast(unsigned char, *endptr))) endptr++;\n}\n";
    check(input, &["--style=kr", "--align-pointer=middle"], input);
    let input = "void f(void)\n{\n    if (type & *want_type && x)\n        puts(x);\n}\n";
    check(input, &["--style=kr", "--align-pointer=type"], input);
}

#[test]
fn max_code_length_takes_no_split_point_within_ten_columns_of_the_code() {
    let input = "void f(void)\n{\n    x = g(a, b);                    /* Next chunk in the journal */\n    xx = gg(aaa, bbbb,\n            cc);            // Next chunk in the journal\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], input);
}

#[test]
fn max_code_length_keeps_a_string_literal_after_its_call_paren() {
    let input = "void f(void)\n{\n    if (x) {\n        curl_mprintf(\"CURLUPART_SCHEME %d bytes scheme == %d (%s)\\n\",\n                     EXCESSIVE, (int)uc, curl_url_strerror(uc));\n    }\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], input);
}

#[test]
fn returned_value_operand_row_under_a_code_length_limit_stands_at_the_value() {
    let input = "void f(void)\n{\n    if (x) {\n        return ngx_snprintf(text, len, \"%ud.%ud.%ud.%ud\",\n                            p[0], p[1], p[2], p[3])\n               - text;\n    }\n}\n";
    check(input, &["--style=1tbs", "--max-code-length=109"], input);
}

#[test]
fn parameters_after_an_indented_preprocessor_block_start_from_the_function_line() {
    let input = "#ifdef _WIN32\n    __declspec(dllexport)\n#endif\nint sqlite3_randomjson_init(\n    sqlite3 *db,\n    char **pzErrMsg\n)\n{\n    return 0;\n}\n";
    check(input, &["--style=allman", "--indent-preproc-block"], input);
}

#[test]
fn preprocessor_block_opened_by_an_error_directive_stays_unindented() {
    let input = "#ifdef X\n#error This file.\n#endif\n";
    check(input, &["--style=allman", "--indent-preproc-block"], input);
}

#[test]
fn statements_of_blocks_in_a_broken_else_if_chain_keep_the_first_column() {
    let input = "void f() {\n    if (a) {\n        x = 1;\n        }\n    else\n        if (b) {\n            y = 1;\n            }\n        else\n            if (c) {\n                int i;\n                i = g(p);\n                p->n++;\n                }\n            else {\n                p->a = y;\n                p->b = 0;\n                }\n    }\n";
    check(input, &["--style=ratliff", "--break-elseifs"], input);
}

#[test]
fn braceless_else_ending_a_broken_else_if_chain_returns_to_the_chain_head() {
    let input = "void f()\n{\n    if (a)\n        return 1;\n    else\n        if (b)\n            return 2;\n        else\n            if (c)\n                {\n                x = 1;\n                }\n            else\n                name = m;\n\n    for (;;)\n        return fn;\n}\n";
    check(input, &["--style=vtk", "--break-elseifs"], input);
}

#[test]
fn comment_in_a_block_under_a_broken_else_if_chain_keeps_every_chain_level() {
    let input = "void f()\n{\n    if (!argv[0])\n        {\n        }\n    else\n        if (!strcmp(argv[0], \"main\"))\n            {\n            }\n        else\n            if (skip_prefix(argv[0], \"submodule:\", &gitdir))\n                {\n                    for (p = worktrees; *p; p++)\n                        {\n                            if (!wt->id)\n                                {\n                                    /* special case for main worktree */\n                                    if (!strcmp(gitdir, \"main\"))\n                                        break;\n                                }\n                            else\n                                if (!strcmp(gitdir, wt->id))\n                                    break;\n                        }\n                }\n}\n";
    check(input, &["--style=gnu", "--break-elseifs"], input);
}

#[test]
fn define_continuations_past_the_maximum_take_two_indents_from_their_row() {
    let input = "#define xcalloc(nmemb, size) xcalloc_impl(nmemb, size, __FILE__, __LINE__, \\\n        __XMALLOC_FUNCTION)\n#define luaS_newliteral(L, s)\t(luaS_newlstr(L, \"\" s, \\\n                                 (sizeof(s)/sizeof(char))-1))\n#define ISCOEFFZERO(u) (                                      \\\n        UBTOUI((u)+DECPMAX-4)==0                                 \\\n        && UBTOUS((u)+DECPMAX-6)==0                                 \\\n        && *(u)==0)\n#define LONGNAME_FOR_TESTING_PURPOSES(a, b) (fooooo(a, \\\n        b) + barrrrrr(a, \\\n                      b))\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
    let input = "#define X(a) \\\n    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(a, \\\n            b) + cccccccc(dddddddddddddddddddddddddddddddddddddddddddddd(a, \\\n                          b, \\\n                          c))\n#define Y(a) \\\n    aaaaaaaaaaaaaaaaaaaaaaaaaaaa(a, \\\n                                 b) + cccccccc(ddddddd(a, \\\n                                         b, \\\n                                         c))\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
}

#[test]
fn define_rows_after_an_assignment_align_at_its_value_outside_parens() {
    let input = "#define A(x) \\\n    x = foo(a, \\\n            b) + \\\n        c\n#define B(x) \\\n    x = \\\n        foo(a, \\\n            b)\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
}

#[test]
fn define_headers_after_braces_indent_their_bodies() {
    let input = "#define RB_ROTATE_LEFT(head, elm, tmp, field) do {\t\t\t\\\n        (tmp) = RB_RIGHT(elm, field);\t\t\t\t\t\\\n        if (RB_PARENT(elm, field)) {\t\t\t\t\t\\\n            RB_LEFT(RB_PARENT(elm, field), field) = (tmp);\t\t\\\n        } else\t\t\t\t\t\t\t\t\\\n            (head)->rbh_root = (tmp);\t\t\t\t\\\n        RB_LEFT(tmp, field) = (elm);\t\t\t\t\t\\\n    } while (0)\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
    let input = "#define markobject(g,t) { if (iswhite(obj2gco(t))) \\\n            reallymarkobject(g, obj2gco(t)); }\n#define M2(g,t) if (iswhite(obj2gco(t))) \\\n        reallymarkobject(g, obj2gco(t));\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
    let input = "#define POST_COMPLETION_FOR_REQ(loop, req)                              \\\n    if (!PostQueuedCompletionStatus((loop)->iocp,                         \\\n                                    0,                                    \\\n                                    0,                                    \\\n                                    &((req)->u.io.overlapped))) {         \\\n        uv_fatal_error(GetLastError(), \"PostQueuedCompletionStatus\");       \\\n    }\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
}

#[test]
fn vtk_define_closing_brace_is_indented_while_a_block_stays_open() {
    let input = "#define C(x) \\\n    if (x) {\\\n        f(x);\\\n        } else {\\\n        g(x);\\\n    }\n#define D(x) \\\n    if (x) {\\\n        f(x);\\\n    } else\\\n        g(x);\n";
    check(input, &["--style=vtk", "--indent-preproc-define"], input);
}

#[test]
fn define_header_conditions_continue_at_the_minimum_conditional_indent() {
    let input = "#define F(k) \\\n    for (k = 0; k < ARRAY_SIZE(ut_table); \\\n            k++)\n#define G(k) \\\n    for (k = 0, \\\n            j = 1; k < 3; \\\n            k++)\n#define H(c) { \\\n        if( c<0x80 \\\n                || (c&0xFFFFF800)==0xD800 \\\n                || (c&0xFFFFFFFE)==0xFFFE ){  c = 0xFFFD; } \\\n    }\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
}

#[test]
fn define_rows_after_a_paren_ending_its_row_align_at_the_backslash() {
    let input = "#define I(c) do { \\\n        malloc_printf( \\\n                       \"x\", \\\n                       c); \\\n    } while (0)\n#define J(c) do { \\\n        malloc_printf(a, \\\n                      \"x\", \\\n                      c); \\\n    } while (0)\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
}

#[test]
fn define_labels_stand_at_the_body_or_a_level_out_with_indented_labels() {
    let input = "#define R(x) do { \\\n        if (x) { \\\n            f(); \\\n    lbl: \\\n            g(); \\\n        } \\\n    color: \\\n        h(); \\\n    } while (0)\n";
    check(input, &["--indent-preproc-define"], input);
    let input = "#define R(x) do { \\\n        if (x) { \\\n            f(); \\\n        lbl: \\\n            g(); \\\n        } \\\n    color: \\\n        h(); \\\n    } while (0)\n";
    check(
        input,
        &["--indent-preproc-define", "--indent-labels"],
        input,
    );
}

#[test]
fn indent_after_parens_continues_conditions_assignments_and_declarations() {
    let input = "void f()\n{\nagain:\n    if (line->len >= 2 &&\n        !memcmp(line->buf, \"--\", 2))\n        g();\ndone:\n    if (r->request_body_no_buffering\n        && (rc == NGX_OK || rc == NGX_AGAIN)) {\n        g();\n    }\n    if(data->state.http_host &&\n        /* a Host: header */\n        curlx_str_casecompare(&name, \"Host\"))\n        ;\n    ngx_uint_t               i, default_server, proxy_protocol,\n                             protocols, protocols_prev;\n    int mapped = !filter->ipv6_v6only &&\n        is_ipv4_mapped_ipv6_address(address->family, cinaddr);\n}\n";
    check(input, &["--style=kr", "--indent-after-parens"], input);
}

#[test]
fn max_code_length_measures_a_run_in_line_from_its_brace() {
    let input = "void f()\n{   if (x)\n    {   if (y)\n        {   printf(\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", argv[1],\n                   sqlite3_errmsg(db));\n            return;\n        }\n    }\n}\nvoid f()\n{   if (x)\n    {   if (y)\n        {   printf(\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\n                   argv[1], sqlite3_errmsg(db));\n            return;\n        }\n    }\n}\n";
    check(input, &["--style=horstmann", "--max-code-length=60"], input);
}

#[test]
fn pad_operators_keeps_dereferences_after_sizeof_and_parenthesized_names() {
    let input = "int f(int *np, char ***argv, LONG *expected, const char *zSql)\n{\n    if ((u_long) *np >= x) {\n        return 1;\n    }\n    *argv = xreallocarray(*argv, 1, sizeof **argv);\n    if (old == (LONG) *expected) return 1;\n    if( IdChar((u8)*zSql) ) {\n        return 2;\n    }\n    return 0;\n}\n";
    check(input, &["--style=kr", "--pad-oper"], input);
}

#[test]
fn break_blocks_add_no_blank_between_a_comment_and_its_closing_header() {
    let input = "void f()\n{\n    x = 1;\n\n    if (symbol < 16)\n        lengths[index++] = symbol;\n    else {\n        len = 0;\n    }\n\n    y = 2;\n\n    if (a) {\n        /* comment */\n    } else {\n        z();\n    }\n\n    if (b)\n        c();\n    /* else */\n    else if (d)\n        e();\n}\n";
    check(input, &["--style=kr", "--break-blocks"], input);
    let input = "void f()\n{\n    x = 1;\n\n    if (symbol < 16)\n        lengths[index++] = symbol;\n\n    else {\n        len = 0;\n    }\n\n    y = 2;\n\n    if (a) {\n        /* comment */\n    } else {\n        z();\n    }\n\n    if (b)\n        c();\n\n    /* else */\n    else if (d)\n        e();\n}\n";
    check(input, &["--style=kr", "--break-blocks=all"], input);
}

#[test]
fn break_all_blocks_keeps_a_one_line_statement_body_with_its_else() {
    let input = "void f()\n{   for (i = 0; i < n; i++)\n    {   if (type == TCP)\n            a();\n        else\n            b(); }\n\n    x = 1;\n\n    if (c)\n        d();\n    else\n        e(); }\n";
    check(input, &["--style=pico", "--break-blocks=all"], input);
}

#[test]
fn remove_comment_prefix_keeps_double_star_and_tabbed_rows_of_trailing_comments() {
    let input = "struct s {\n    int nArg;          /* Number of arguments */\n    int aIdx[7];           /* Constraints on start, stop, step, LIMIT, OFFSET,\n                         ** and value.  aIdx[5] covers value=, value>=, and\n                         ** value>,  aIdx[6] covers value<= and value< */\n    unsigned oid_valid : 1;  /* if true, use oid and trust mode;\n\t\t\t\t   if false, use the name and read from\n\t\t\t\t   the filesystem.\n*/\n};\n";
    check(input, &["--style=kr", "--remove-comment-prefix"], input);
}

#[test]
fn braces_inside_block_comments_stay_on_their_lines() {
    let input = "/*\n    Example:\n\n       if (eax == [m]) {\n           zf = 1;\n           [m] = r;\n       } else {\n           zf = 0;\n       }\n\n       void foo(void)\n       {\n           warning(\"x\");\n       }\n*/\nint x;\n";
    for style in ["--style=lisp", "--style=pico", "--style=horstmann"] {
        check(input, &[style, "--remove-comment-prefix"], input);
    }
}

#[test]
fn define_assignments_register_until_a_comma_in_their_parens() {
    let input = "#define B(x) \\\n    x = \\\n        foo(a, \\\n            b)\n#define C(x) \\\n    x =   \\\n          foo\n#define S(x) \\\n    (block[n] = \\\n                (uint32_t)data[n * 4] | \\\n                ((uint32_t)data[n * 4 + 1] << 8))\n#define blk0(i) (block->l[i] = (rol(block->l[i],24)&0xFF00FF00) \\\n                               |(rol(block->l[i],8)&0x00FF00FF))\n#define W(p, s) \\\n    ((p)[0] = (u_char) ((s) >> 8), \\\n     (p)[1] = (u_char)  (s), \\\n     (p) + 2)\n";
    check(input, &["--indent-preproc-define"], input);
}

#[test]
fn delete_empty_lines_keeps_the_empty_lines_of_arrays() {
    let input = "int f(void)\n{\n    static const char *a[] = {\n        \"0.0\", \"0.1\",\n\n        \"1.0\", \"1.1\",\n    };\n    struct s x[] = {\n        { 1, 2 },\n\n        { 3, 4 },\n    };\n    return 0;\n}\nstatic const char *b[] = {\n    \"0.0\",\n\n    \"1.0\",\n};\n";
    check(input, &["--style=kr", "--delete-empty-lines"], input);
}

#[test]
fn rows_of_a_comment_opened_after_code_keep_their_whitespace_with_tabs() {
    let input = "struct s {\n\tint seekResult;         /* Result of previous\n                          ** if there have been. */\n\tint x; /* a\n              b */\n};\nvoid f()\n{\n\tif (x) {\n\t\tg(); /* so the loop can exit;\n                we *shouldn* get */\n\t}\n}\n";
    check(input, &["--style=kr", "--indent=force-tab=4"], input);
    check(input, &["--style=kr", "--indent=tab=4"], input);
}

#[test]
fn name_aligned_pointers_leave_logical_and_multiplication_and_returns_alone() {
    let input = "int f(fd_set *fds_read, int iPhrase)\n{\n    aOut[iPhrase * ((p->nCol+31)/32) + iCol/32] |= 1;\n    return select((int)maxfd + 1,\n                  fds_read && fds_read->fd_count ? fds_read : NULL,\n                  ptimeout);\n}\nstatic inline int weight(struct commit_list *elem)\n{\n    return **commit_weight_at(&commit_weight, elem->item);\n}\nvoid f()\n{\n    x = a * ((b));\n    y[a * ((b))] = 1;\n    y[a * (b)] = 1;\n    z = y[a * b];\n    y[iPhrase * ((p->nCol+31)/32) + iCol/32] |= 1;\n}\n";
    check(input, &["--style=1tbs", "--align-pointer=name"], input);
}

#[test]
fn vtk_indents_a_one_line_block_brace_within_a_block() {
    let input = "void f()\n{\n    for (;;)\n        {\n        if (x)\n            { a = 1; }\n        else\n            { b = 2; }\n        }\n    if (y)\n    { c = 1; }\n}\n";
    check(input, &["--style=vtk", "--add-one-line-braces"], input);
    check(input, &["--style=vtk", "--keep-one-line-blocks"], input);
}

#[test]
fn indented_conditionals_in_a_case_block_stand_at_its_body() {
    let input = "int f(int id)\n{\n    switch (id) {\n    default: {\n        #ifdef SQLITE_ENABLE_API_ARMOR\n        if (id < 0) {\n            return 0;\n        }\n        #endif\n        break;\n    }\n    }\n    return 1;\n}\n";
    check(input, &["--style=kr", "--indent-preproc-cond"], input);
}

#[test]
fn two_case_labels_on_a_line_own_the_block_after_them() {
    let input = "int f(int option)\n{\n    switch (option) {\n    case 1:  case 2: {\n        int status;\n        if (x) return 0;\n        break;\n    }\n    default:\n        break;\n    }\n    return 0;\n}\n";
    check(input, &["--style=kr", "--keep-one-line-statements"], input);
}

#[test]
fn case_block_content_takes_another_level_with_indented_cases() {
    let input = "int f(int q)\n{\n    switch(q) {\n    case 1: {\n            if(x) {\n                return 1;\n            }\n            break;\n        }\n    case 2:\n        break;\n    }\n    return 0;\n}\n";
    check(input, &["--style=linux", "--indent-cases"], input);
}

#[test]
fn reference_arguments_in_one_line_blocks_stay_attached() {
    let input = "void f()\n{\n    if (x) { g(ac, &cb, NULL); }\n    if (y)\n    { g(ac, &cb, NULL); }\n    { g(ac, &cb, NULL); }\n}\n";
    check(
        input,
        &[
            "--style=allman",
            "--keep-one-line-blocks",
            "--align-pointer=middle",
        ],
        input,
    );
}

#[test]
fn define_statement_after_a_trailing_assignment_aligns_as_astyle_registers_it() {
    let input = "#define SLIST_REMOVE(head, elm, type, field) do {\t\t\t\\\n        if ((head)->slh_first == (elm)) {\t\t\t\t\\\n            SLIST_REMOVE_HEAD((head), field);\t\t\t\\\n        } else {\t\t\t\t\t\t\t\\\n            struct type *curelm = (head)->slh_first;\t\t\\\n            curelm->field.sle_next =\t\t\t\t\\\n                                                    curelm->field.sle_next->field.sle_next;\t\t\\\n            (listelm)->field.le_next->field.le_prev =\t\t\\\n                    &(elm)->field.le_next;\t\t\t\t\\\n        }\t\t\t\t\t\t\t\t\\\n    } while (0)\n";
    check(input, &["--style=kr", "--indent-preproc-define"], input);
}

#[test]
fn indented_preprocessor_blocks_keep_error_lines_as_written_and_indent_comments() {
    let input = "int x;\n#if A!=B\n    # error wrong\n#endif\n#ifdef USE_WINSOCK\n#elif defined(__AMIGA__) /* Any AmigaOS flavor */\n    /* long recv(long, char *, long, long); */\n    #define RECV_TYPE_ARG1 long\n#endif\n";
    check(input, &["--indent-preproc-block"], input);
}

#[test]
fn case_blocks_under_a_split_else_keep_indented_case_levels() {
    let input = "int main(void)\n{\n    if( a ) {\n        x();\n    } else\n\n        /* changeset FILE sql\n        ** Show\n        */\n        if( b ) {\n            switch( op ) {\n            case 1: {\n                    int i;\n                    break;\n                }\n            }\n        }\n    return 0;\n}\n";
    check(input, &["--style=linux", "--indent-cases"], input);
    // A comment opener inside a string is no comment.
    let input = "int main(void)\n{\n    if( a ) {\n        x();\n    } else\n\n        /* c */\n        if( b ) {\n            switch( op ) {\n            case 1: {\n                    int i;\n                    printf(\"/* %d */ x\");\n                    for(i=0; i<n; i++) {\n                        y();\n                    }\n                    break;\n                }\n            }\n        }\n    return 0;\n}\n";
    check(input, &["--style=linux", "--indent-cases"], input);
}

#[test]
fn reference_to_a_parenthesized_declarator_keeps_its_spacing() {
    let input = "class A\n{\n    gzomanip2(gzofstream& (*f)(gzofstream &, T1, T2),\n              T1 v1);\n    void g(char *(*f)(int));\n    void h(char *(*f)(int));\n};\n";
    check(input, &["--style=1tbs", "--align-reference=name"], input);
    check(input, &["--style=1tbs", "--align-pointer=name"], input);
}

#[test]
fn initializer_member_after_a_directive_stands_at_the_previous_member_start() {
    let input =
        "static T a[] = {\n    W(s, 2,\n      g),\n#ifdef X\n    W(m, 1,\n      p),\n#endif\n};\n";
    check(input, &[], input);
}

#[test]
fn enum_run_in_after_its_brace_indents_a_block_level_after_parens() {
    let input = "struct s {\n    enum e { A,\n        B, C,\n        D\n    };\n    int x;\n};\ntypedef enum { A,\n    B\n} t;\nenum { A = 1,\n    B = (2 +\n            3),\n    C = f(2,\n            3)\n} y;\n";
    check(input, &["--indent-after-parens"], input);
    let input =
        "enum {\n    A = 1,\n    B = (2 +\n         3),\n    C = f(2,\n          3)\n} x;\n";
    check(input, &[], input);
}

#[test]
fn statement_led_by_a_call_or_cast_continues_nothing_past_its_comma() {
    let input = "int f(int a)\n{\n    (void)file, (void)a,\n    (void)b;\n    g(a),\n    c;\n    int d(1),\n        e;\n    return 0;\n}\n";
    check(input, &[], input);
    check(input, &["--indent-after-parens"], input);
}

#[test]
fn ternary_arm_in_parens_stacks_after_parens() {
    let input = "void f(void)\n{\n    show_ce(repo, dir,\n        ce_stage(ce) ? tag_unmerged :\n        (ce_skip_worktree(ce) ? tag_skip_worktree :\n            tag_cached));\n}\n";
    check(input, &["--indent-after-parens"], input);
}

#[test]
fn assignment_split_by_a_directive_stacks_after_parens() {
    let input = "void f(void)\n{\n    while (len--) {\n        s->out[s->outcnt] =\n#ifdef A\n            dist > s->outcnt ?\n            0 :\n#endif\n            s->out[s->outcnt - dist];\n        s->outcnt++;\n    }\n}\n";
    check(input, &["--indent-after-parens"], input);
}

#[test]
fn conditional_directive_in_a_header_stands_at_its_level_after_parens() {
    let input = "void f(void)\n{\n    if(a ||\n    #ifdef K\n        b ||\n    #endif\n        c)\n        g();\n    if(x) {\n        h(a,\n            #ifdef K\n            b,\n            #endif\n            c);\n    }\n}\n";
    check(
        input,
        &["--indent-preproc-cond", "--indent-after-parens"],
        input,
    );
}

#[test]
fn first_shift_stacks_like_a_paren_after_parens() {
    let input = "void f(void)\n{\n  cout << a <<\n    b;\n  cout << a\n    << b;\n  t = a << 16 |\n    b;\n  x = cout << a <<\n    b;\n}\n";
    check(
        input,
        &["--indent=spaces=2", "--indent-after-parens"],
        input,
    );
}

#[test]
fn define_rows_stack_parens_and_return_after_parens() {
    let input = "#define X(a_prefix) \\\n    a_type *\\\n    a_prefix##_first(a_prefix##_t *ph) {\\\n        return ph_first(&ph->ph, offsetof(a_type, a_field),\\\n                &a_prefix##_ph_cmp);\\\n    }\\\n    void g(void) {\\\n        ph_insert(&ph->ph, phn,\\\n            a_prefix##_ph_cmp);\\\n    }\n#define Y(a) \\\n    return a +\\\n        b;\n";
    check(
        input,
        &["--indent-preproc-define", "--indent-after-parens"],
        input,
    );
}

#[test]
fn define_return_continues_at_its_value() {
    let input = "#define X(a) \\\n    return a +\\\n           b;\n";
    check(input, &["--indent-preproc-define"], input);
}

#[test]
fn max_code_length_keeps_initializer_continuation_rows_whole() {
    let input = "static struct option opts[] = {\n    OPT_CMDMODE_F(0, \"config-sections\", &cmd_mode, \"\", HELP_ACTION_CONFIG_SECTIONS_FOR_COMPLETION, PARSE_OPT_HIDDEN),\n    OPT_CMDMODE_F(0, \"config-sections\", &cmd_mode, \"\",\n        HELP_ACTION_CONFIG_SECTIONS_FOR_COMPLETION, PARSE_OPT_HIDDEN),\n};\n";
    let expected = "static struct option opts[] = {\n    OPT_CMDMODE_F(0, \"config-sections\", &cmd_mode, \"\", HELP_ACTION_CONFIG_SECTIONS_FOR_COMPLETION, PARSE_OPT_HIDDEN),\n    OPT_CMDMODE_F(0, \"config-sections\", &cmd_mode, \"\",\n                  HELP_ACTION_CONFIG_SECTIONS_FOR_COMPLETION, PARSE_OPT_HIDDEN),\n};\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn max_code_length_splits_no_unpadded_bitwise_or_multiplicative_operator() {
    let input = "void f(void)\n{\n    chmod(dst, S_IRUSR|S_IRGRP|S_IROTH|S_IRUSR|S_IRGRP|S_IROTH|S_IRUSR|S_IRGRP);\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa*bbbbbbbbbbbbbbbbbbbbbbbbbbb*ccccccccccc);\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa<bbbbbbbbbbbbbbbbbbbbbbbbbbb<ccccccccccc);\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa+bbbbbbbbbbbbbbbbbbbbbbbbbbb+ccccccccccc);\n}\n";
    let expected = "void f(void)\n{\n    chmod(dst,\n          S_IRUSR|S_IRGRP|S_IROTH|S_IRUSR|S_IRGRP|S_IROTH|S_IRUSR|S_IRGRP);\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa*bbbbbbbbbbbbbbbbbbbbbbbbbbb*ccccccccccc);\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa<bbbbbbbbbbbbbbbbbbbbbbbbbbb<ccccccccccc);\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n         +bbbbbbbbbbbbbbbbbbbbbbbbbbb+ccccccccccc);\n}\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn max_code_length_operator_split_in_parens_after_assignment_aligns_at_the_paren() {
    let input = "void f(void)\n{\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa==bbbbbbbbbbbbbbbbbbbbbbbbbbb==ccccccccccc);\n    x = g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbb + ccccccccccc);\n    x = aaaaaaaaaaaaaaaaa + g(bbbbbbbbbbbbbbbbbbbbbbbbbbb) + ccccccccccccccccc;\n}\n";
    let expected = "void f(void)\n{\n    x = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaa==\n         bbbbbbbbbbbbbbbbbbbbbbbbbbb==ccccccccccc);\n    x = g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaa +\n          bbbbbbbbbbbbbbbbbbbbbbbbbbb + ccccccccccc);\n    x = aaaaaaaaaaaaaaaaa + g(bbbbbbbbbbbbbbbbbbbbbbbbbbb) +\n        ccccccccccccccccc;\n}\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn max_code_length_splits_a_declaration_before_its_pointer_at_the_statement() {
    let input = "void f(void)\n{\n    FileChunk *pNext; /* Next chunk in the journal xxxxxxxxxxxxxxxxxxx */\n    FileChunkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk *pNexttttttttttttttttttttttttttt;\n    FileChunkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk *pNextttttttttttt = gggggggggggggggggggg;\n    struct FileChunkkkkkkkkkkkkkkkkkkkkkkkkkk *pNextttttttttttttttttttttt(int a);\n    unsigned int xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;\n    static const struct FileChunkkkkkkkkkkkkkkkkkkkk pNextttttttttttttttttttttt;\n}\nstruct F\n{\n    FileChunk *pNext;               /* Next chunk in the journal */\n    long long stats_bus_messages_received[CLUSTERMSG_TYPE_COUNT];\n};\n";
    let expected = "void f(void)\n{\n    FileChunk *pNext; /* Next chunk in the journal xxxxxxxxxxxxxxxxxxx */\n    FileChunkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk\n    *pNexttttttttttttttttttttttttttt;\n    FileChunkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk *pNextttttttttttt =\n        gggggggggggggggggggg;\n    struct FileChunkkkkkkkkkkkkkkkkkkkkkkkkkk\n    *pNextttttttttttttttttttttt(int a);\n    unsigned int\n    xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx;\n    static const struct FileChunkkkkkkkkkkkkkkkkkkkk\n        pNextttttttttttttttttttttt;\n}\nstruct F\n{\n    FileChunk *pNext;               /* Next chunk in the journal */\n    long long stats_bus_messages_received[CLUSTERMSG_TYPE_COUNT];\n};\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn max_code_length_splits_after_no_bracket() {
    let input = "void f(void)\n{\n    xxxxxxxxxxxxxxxxxxxxxxx = stats_bus_messages_received[CLUSTERMSG_TYPE_COUNT];\n}\n";
    let expected = "void f(void)\n{\n    xxxxxxxxxxxxxxxxxxxxxxx =\n        stats_bus_messages_received[CLUSTERMSG_TYPE_COUNT];\n}\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn leading_assignment_before_a_block_initializer_stands_at_its_statement() {
    let input = "static ngx_http_module_t  ngx_http_headers_filter_module_ctx\n=\n{\n    NULL,\n    ngx_http_headers_filter_init,\n};\nvoid f(void)\n{\n    static ngx_http_module_t  ctx\n    =\n    {\n        NULL,\n    };\n    int a[]\n        = { 1, 2 };\n}\n";
    let expected = "static ngx_http_module_t  ngx_http_headers_filter_module_ctx\n=\n{\n    NULL,\n    ngx_http_headers_filter_init,\n};\nvoid f(void)\n{\n    static ngx_http_module_t  ctx\n    =\n    {\n        NULL,\n    };\n    int a[]\n        = { 1, 2 };\n}\n";
    check(input, &["--style=allman"], expected);
    check(expected, &["--style=allman"], expected);
}

#[test]
fn function_brace_after_a_parameter_split_before_its_pointer_stays_at_the_function() {
    let input = "static int remove_available_paths(struct string_list_item\n                                  *item, void *cb_data)\n{\n    struct string_list *available_paths = cb_data;\n    return 0;\n}\n";
    let expected = "static int remove_available_paths(struct string_list_item\n                                  *item, void *cb_data)\n{\n    struct string_list *available_paths = cb_data;\n    return 0;\n}\n";
    check(input, &["--style=allman"], expected);
    check(expected, &["--style=allman"], expected);
}

#[test]
fn continued_conditional_in_an_indented_block_continues_a_level_past_it() {
    let input = "int x;\n#ifdef U\n#include <a.h>\n#if B || \\\n  C\n#define X 1\n#elif C || \\\n  D\n#define X 2\n#endif\n#endif\n";
    let expected = "int x;\n#ifdef U\n    #include <a.h>\n    #if B || \\\n        C\n        #define X 1\n    #elif C || \\\n        D\n        #define X 2\n    #endif\n#endif\n";
    check(input, &["--indent-preproc-block"], expected);
    check(expected, &["--indent-preproc-block"], expected);
}

#[test]
fn conditional_with_parens_across_its_lines_leaves_its_blocks_unindented() {
    let input = "int x;\n#ifdef U\n#if (B || \\\n  C)\n#define X 1\n#endif\n#endif\n#ifdef V\n#  include <a.h>\n#  if A ||  \\\n     (B &&  \\\n      C)\n#    define Y 1\n#  endif\n#endif\n";
    let expected = "int x;\n#ifdef U\n#if (B || \\\n  C)\n#define X 1\n#endif\n#endif\n#ifdef V\n#  include <a.h>\n#  if A ||  \\\n     (B &&  \\\n      C)\n#    define Y 1\n#  endif\n#endif\n";
    check(input, &["--indent-preproc-block"], expected);
    check(expected, &["--indent-preproc-block"], expected);
}

#[test]
fn indented_preprocessor_block_indents_column_one_comments() {
    let input = "int x;\n#if A && \\\n  B\n/* c */\n#define T 50\n// d\n#endif\n";
    let expected = "int x;\n#if A && \\\n    B\n    /* c */\n    #define T 50\n    // d\n#endif\n";
    check(input, &["--indent-preproc-block"], expected);
    check(expected, &["--indent-preproc-block"], expected);
}

#[test]
fn type_with_a_star_before_a_comma_in_parens_is_a_pointer() {
    let input = "void f(void)\n{\n    x = GLOBAL(BtShared*, y);\n    x = GLOBAL(BtShared *, y);\n    h(sizeof(BtShared*), a*b);\n}\n";
    let expected = "void f(void)\n{\n    x = GLOBAL(BtShared*, y);\n    x = GLOBAL(BtShared *, y);\n    h(sizeof(BtShared*), a * b);\n}\n";
    check(input, &["--pad-oper"], expected);
    check(expected, &["--pad-oper"], expected);
}

#[test]
fn type_with_a_star_before_a_comma_aligns_as_a_pointer() {
    let input = "void f(void)\n{\n    x = GLOBAL(BtShared*, y);\n}\n";
    let expected = "void f(void)\n{\n    x = GLOBAL(BtShared *, y);\n}\n";
    check(input, &["--pad-oper", "--align-pointer=name"], expected);
    check(expected, &["--pad-oper", "--align-pointer=name"], expected);
}

#[test]
fn row_after_a_bracket_ending_its_line_continues_a_level_past_the_indent() {
    let input = "void f(void)\n{\n    char x[\n  A + 2];\n    y = a[\n  A + 2];\n    g(a[\n  A + 2]);\n}\nint y = a[\n  A + 2];\n";
    let expected = "void f(void)\n{\n    char x[\n        A + 2];\n    y = a[\n            A + 2];\n    g(a[\n          A + 2]);\n}\nint y = a[\n            A + 2];\n";
    check(input, &[], expected);
    check(expected, &[], expected);
}

#[test]
fn fill_empty_lines_keeps_form_feeds_and_skips_continued_directive_braces() {
    let input = "#define R { \\\n\t.e = S, \\\n}\n\nint f(void)\n{\n    x();\n\n    y();\n\x0c\n    z();\n}\n";
    let expected = "#define R { \\\n\t.e = S, \\\n}\n\nint f(void)\n{\n    x();\n    \n    y();\n\x0c\n    z();\n}\n";
    check(input, &["--fill-empty-lines"], expected);
    let input = "int f(void)\n{\n    int x;\n    /* Test that scheme is properly initialized.\n     */\n\n    result = g();\n}\n";
    let expected = "int f(void) {\n    int x;\n    /* Test that scheme is properly initialized.\n     */\n    \n    result = g(); }\n";
    check(input, &["--style=lisp", "--fill-empty-lines"], expected);
}

#[test]
fn spaces_before_a_comma_after_code_are_dropped() {
    let input = "int f(int , int *);\nvoid g(void)\n{\n    h( , b);\n    h(a /* c */ , b);\n    x = a , b;\n    h(a\n      , b);\n    h(a\t, b);\n}\nenum e { A , B };\n";
    let expected = "int f(int, int *);\nvoid g(void)\n{\n    h(, b);\n    h(a /* c */, b);\n    x = a, b;\n    h(a\n      , b);\n    h(a\t, b);\n}\nenum e { A, B };\n";
    check(input, &[], expected);
}

#[test]
fn joined_closing_header_shrinks_the_gap_before_its_trailing_comment() {
    let input = "void f(void)\n{\n  if (a) {\n    x();\n  }\n  else {  /* out = \"source\" */\n    y();\n  }\n  if (a) {\n    x();\n  }\n  else if (b) {      /* Check */\n    y();\n  }\n}\n";
    let expected = "void f(void)\n{\n    if (a) {\n        x();\n    } else { /* out = \"source\" */\n        y();\n    }\n    if (a) {\n        x();\n    } else if (b) {    /* Check */\n        y();\n    }\n}\n";
    check(input, &["--style=kr"], expected);
}

#[test]
fn comment_after_a_broken_brace_keeps_its_source_column() {
    let input = "void f(void)\n{\n    if (a) {\n        x();\n    } else {     /* c */\n        y();\n    }\n    if (a) {\n        x();\n    } else if (b) {      /* d */\n        y();\n    }\n}\n";
    let expected = "void f(void)\n{\n    if (a)\n    {\n        x();\n    }\n    else         /* c */\n    {\n        y();\n    }\n    if (a)\n    {\n        x();\n    }\n    else if (b)          /* d */\n    {\n        y();\n    }\n}\n";
    check(input, &["--style=allman"], expected);
}

#[test]
fn comment_after_a_case_label_brace_stays_on_the_label_line() {
    let input = "void f(int c)\n{\n  switch (c) {\n    case 1: {  /* start capture */\n      x();\n      break;\n    }\n    case 2: { // line c\n      y();\n    }\n  }\n}\n";
    let expected = "void f(int c)\n{\n    switch (c)\n    {\n    case 1:    /* start capture */\n    {\n        x();\n        break;\n    }\n    case 2:   // line c\n    {\n        y();\n    }\n    }\n}\n";
    check(input, &["--style=allman"], expected);
}

#[test]
fn control_header_indexing_an_array_breaks_its_one_line_block() {
    let input =
        "int f(void)\n{\n  while( n>0 && p->aAction[n-1].lookahead<0 ){ n--; }\n  return n;\n}\n";
    let expected = "int f(void)\n{\n    while( n>0 && p->aAction[n-1].lookahead<0 ) {\n        n--;\n    }\n    return n;\n}\n";
    check(input, &["--style=kr"], expected);
}

#[test]
fn label_after_a_comment_line_splits_from_its_statement() {
    let input = "void f(int c)\n{\n  switch (c) {\n    case 1:\n      x();\n      /* go through */\n    no_save: break;\n  }\n  x();\n  /* c */\nlbl: y();\n}\n";
    let expected = "void f(int c)\n{\n    switch (c) {\n    case 1:\n        x();\n        /* go through */\nno_save:\n        break;\n    }\n    x();\n    /* c */\nlbl:\n    y();\n}\n";
    check(input, &["--style=kr"], expected);
}

#[test]
fn wide_gap_before_a_line_comment_shrinks_by_added_padding() {
    let input =
        "void f(void)\n{\n  a=b;        // c\n  uInt targar[4]={0,0,0,0};        // target\n}\n";
    let expected = "void f(void)\n{\n    a = b;      // c\n    uInt targar[4] = {0, 0, 0, 0};   // target\n}\n";
    check(input, &["--pad-oper"], expected);
}

#[test]
fn else_joined_from_its_own_line_keeps_the_case_block_closing_brace() {
    let input = "void f(void)\n{\n  switch (t) {\n    case 1: {\n\tif (a) {\n\t    x();\n\t}\n\telse {\n\t    y();\n\t}\n\tbreak;\n    }\n  }\n}\n";
    let expected = "void f(void)\n{\n    switch (t) {\n    case 1: {\n        if (a) {\n            x();\n        } else {\n            y();\n        }\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], expected);
}

#[test]
fn comment_led_argument_row_leaves_no_column_past_its_statement() {
    let input = "TEST_BEGIN(t)\n{\n    expect_simple(&tree, /* specialness */ 4, /* empty */ true,\n                  /* first */ NULL, /* last */ NULL);\n}\nTEST_END\n\nint x;\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn added_braces_leave_a_comment_after_a_blank_line_below_the_block() {
    let input = "int f(void)\n{\n    if (malloc_failed) *malloc_failed = 0;\n\n    /* We cannot rehash twice. */\n    assert(x);\n}\n";
    let expected = "int f(void)\n{\n    if (malloc_failed) {\n        *malloc_failed = 0;\n    }\n\n    /* We cannot rehash twice. */\n    assert(x);\n}\n";
    check(input, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

#[test]
fn added_brace_takes_its_place_out_of_the_gap_before_a_header_comment() {
    let input = "void f(void)\n{\n  if (path == NULL)  /* no versioned? */\n    path = getenv(envname);  /* try */\n  if (rupper >= rlower)\t/* cannot be */\n    return;\n  if (b) /* c */\n    return 0;\n}\n";
    let expected = "void f(void)\n{\n    if (path == NULL) { /* no versioned? */\n        path = getenv(envname);    /* try */\n    }\n    if (rupper >= rlower) {\t/* cannot be */\n        return;\n    }\n    if (b) { /* c */\n        return 0;\n    }\n}\n";
    check(input, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

#[test]
fn added_braces_set_the_statement_comment_an_indent_past_it() {
    let input = "void f(void)\n{\n  if (a) return; /* c */\n  if (b)\n    return;      /* d */\n  if (c) { return; } /* e */\n}\n";
    let expected = "void f(void)\n{\n  if (a) {\n    return;  /* c */\n  }\n  if (b) {\n    return;  /* d */\n  }\n  if (c) {\n    return;  /* e */\n  }\n}\n";
    check(input, &["--style=1tbs", "--indent=spaces=2"], expected);
    check(expected, &["--style=1tbs", "--indent=spaces=2"], expected);
}

#[test]
fn added_braces_close_after_a_multi_row_trailing_comment_as_written_braces_do() {
    let written = "void f(void)\n{\n    if (a) {\n        value = 1; /* first\n                      second */\n    }\n    else\n        value = 2;\n}\n";
    let added = "void f(void)\n{\n    if (a)\n        value = 1; /* first\n                      second */\n    else\n        value = 2;\n}\n";
    let expected = "void f(void)\n{\n    if (a) {\n        value = 1; /* first\n                      second */\n    } else {\n        value = 2;\n    }\n}\n";
    check(written, &["--style=1tbs"], expected);
    check(added, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

#[test]
fn broken_one_line_function_body_comment_keeps_the_gap_before_its_brace() {
    let input = "int f(){return 0;}// tail\nint g(){return 0;} // tail\n";
    let expected = "int f() {\n    return 0;   // tail\n}\nint g() {\n    return 0;   // tail\n}\n";
    check(input, &[], expected);
    check(expected, &[], expected);
}

#[test]
fn attach_return_type_sees_only_the_line_before_the_name() {
    let input = "int x;\nTEST_END\nstatic void *\nthd_start(void *varg)\n{\n}\nint y;\nstatic\nvoid *\ng(void *varg)\n{\n}\nint z;\nu_char *\nngx_run(int s)\n{\n}\n";
    let expected = "int x;\nTEST_END\nstatic void *thd_start(void *varg)\n{\n}\nint y;\nstatic\nvoid *g(void *varg)\n{\n}\nint z;\nu_char *ngx_run(int s)\n{\n}\n";
    check(input, &["--attach-return-type"], expected);
    check(expected, &["--attach-return-type"], expected);
}

#[test]
fn padding_parts_code_from_an_adjacent_block_comment() {
    let input = "struct M {\n  char *zMalloc;      /* Space */\n  void (*xDel)(void*);/* Destructor */\n};\n";
    let expected = "struct M {\n    char *zMalloc;      /* Space */\n    void (*xDel)(void *); /* Destructor */\n};\n";
    check(input, &["--align-pointer=name"], expected);
    check(expected, &["--align-pointer=name"], expected);
}

#[test]
fn unpadding_parens_leaves_one_space_before_a_paren() {
    let input = "void f(void)\n{\n    d =  (v / x) / 3;\n    h(a,  (b));\n    return  (x);\n}\n";
    let expected = "void f(void)\n{\n    d = (v / x) / 3;\n    h(a, (b));\n    return (x);\n}\n";
    check(input, &["--unpad-paren"], expected);
    check(expected, &["--unpad-paren"], expected);
}

#[test]
fn initializer_brace_after_its_assignment_line_stands_at_the_statement() {
    let input = "void f(void)\n{\n    static int b[] =\n        { 1, 2 };\n    static int c[] =\n        {\n            1, 2\n        };\n}\n";
    let expected = "void f(void)\n{\n    static int b[] =\n    { 1, 2 };\n    static int c[] =\n    {\n        1, 2\n    };\n}\n";
    check(
        input,
        &["--break-after-logical", "--min-conditional-indent=0"],
        expected,
    );
    check(
        expected,
        &["--break-after-logical", "--min-conditional-indent=0"],
        expected,
    );
}

#[test]
fn unpadding_parens_drops_the_space_after_a_negation() {
    let input =
        "void f(void)\n{\n    if (! (i == argc - 1))\n        x = ~ (c);\n    y = a + (b);\n}\n";
    let expected =
        "void f(void)\n{\n    if(!(i == argc - 1))\n        x = ~(c);\n    y = a + (b);\n}\n";
    check(input, &["--unpad-paren"], expected);
    check(expected, &["--unpad-paren"], expected);
}

#[test]
fn unpadding_parens_keeps_a_space_only_after_astyle_numeric_types() {
    let input = "struct z {\n    unsigned (*m)(int);\n    void (*f)(int);\n    uint8_t (*g)(int);\n};\nLUA_API lua_CFunction (lua_atpanic)(lua_State *L);\nLUA_API int (lua_gettop)(lua_State *L);\n";
    let expected = "struct z {\n    unsigned(*m)(int);\n    void (*f)(int);\n    uint8_t (*g)(int);\n};\nLUA_API lua_CFunction(lua_atpanic)(lua_State *L);\nLUA_API int (lua_gettop)(lua_State *L);\n";
    check(input, &["--unpad-paren"], expected);
    check(expected, &["--unpad-paren"], expected);
}

#[test]
fn argument_after_a_broken_one_line_header_aligns_to_the_call_paren() {
    let input = "void f(void)\n{\n    if( rc ) fatal_error(\"Could not\",\n                         sqlite3_errmsg(db));\n}\n";
    let expected = "void f(void)\n{\n    if( rc )\n        fatal_error(\"Could not\",\n                    sqlite3_errmsg(db));\n}\n";
    let options = [
        "--break-one-line-headers",
        "--break-after-logical",
        "--min-conditional-indent=0",
        "--max-code-length=109",
    ];
    check(input, &options, expected);
    check(expected, &options, expected);
}

#[test]
fn initializer_rows_align_to_the_first_element_past_comments() {
    let input = "void f(void)\n{\n    T arg = {/* h */ !p,\n        /* e */ false, /* fd */ -1};\n    int b[] = {a,\n        /* e */ false, -1};\n    int c[] = {a,\n        false, /* e */ -1};\n}\n";
    let expected = "void f(void)\n{\n    T arg = {/* h */ !p,\n                     /* e */ false, /* fd */ -1\n            };\n    int b[] = {a,\n               /* e */ false, -1\n              };\n    int c[] = {a,\n               false, /* e */ -1\n              };\n}\n";
    for options in [
        &["--pad-oper"][..],
        &["--pad-oper", "--min-conditional-indent=0"][..],
    ] {
        check(input, options, expected);
        check(expected, options, expected);
    }
}

#[test]
fn case_block_initializer_rows_align_to_the_first_element() {
    let input = "int f(int op)\n{\n  switch( op ){\n    case 1: {\n      int *p = (int*)a;\n      static const char *az[] = { \"S\", \"R\",\n                                  \"P\", \"E\" };\n      g(x);\n    }\n  }\n}\n";
    let expected = "int f(int op)\n{\n    switch ( op ) {\n        case 1: {\n            int *p = (int *)a;\n            static const char *az[] = { \"S\", \"R\",\n                                        \"P\", \"E\"\n                                      };\n            g(x);\n        }\n    }\n}\n";
    let options = [
        "--indent-switches",
        "--pad-header",
        "--align-pointer=name",
        "--min-conditional-indent=0",
    ];
    check(input, &options, expected);
    check(expected, &options, expected);
}

#[test]
fn every_style_runs_a_brace_into_a_nested_brace() {
    let input = "static const Node d = {\n  {{NULL}, LUA_VEMPTY,  /* value */\n   LUA_TDEADKEY, 0, {NULL}}  /* key */\n};\n";
    let expected = "static const Node d = {\n    {   {NULL}, LUA_VEMPTY,  /* value */\n        LUA_TDEADKEY, 0, {NULL}\n    }  /* key */\n};\n";
    for style in ["--style=1tbs", "--style=allman", "--style=kr"] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

#[test]
fn column_one_comment_in_a_guarded_struct_takes_the_body_indent() {
    let input = "#ifndef Z\nstruct A {\n  int s;\n// Some\n  int x;\n};\n#endif\n";
    let expected = "#ifndef Z\nstruct A {\n    int s;\n    // Some\n    int x;\n};\n#endif\n";
    check(input, &["--indent-col1-comments"], expected);
    check(expected, &["--indent-col1-comments"], expected);
}

#[test]
fn parameters_follow_a_template_function_name_onto_its_attached_return_type() {
    let input = "template<typename T>\ninline\nA<T1,T2>::A(int f,\n            int v1)\n{ }\ntemplate<typename T>\nstatic int\nf(int f,\n  int v1)\n{ }\n";
    let expected = "template<typename T>\ninline A<T1,T2>::A(int f,\n                   int v1)\n{ }\ntemplate<typename T>\nstatic int f(int f,\n             int v1)\n{ }\n";
    for options in [
        &["--attach-return-type"][..],
        &["--attach-return-type", "--min-conditional-indent=0"][..],
    ] {
        check(input, options, expected);
        check(expected, options, expected);
    }
}

#[test]
fn attached_closing_brace_drops_trailing_source_space() {
    let input = "void f(void)\n{\n  g(); \n}\n";
    check(input, &["--style=pico"], "void f(void)\n{   g(); }\n");
    check(input, &["--style=lisp"], "void f(void) {\n    g(); }\n");
}

#[test]
fn split_else_chain_indent_ends_with_its_block() {
    let input = "void f(void)\n{\n  if( z ){\n    if( a ){\n      x = 1;\n    }else\n#ifdef X\n    if( b ){\n      x = 2;\n    }else\n#endif\n    if( c ){\n      x = 3;\n    }\n  }else if( d ){\n    y = 1;\n  }\n}\n";
    let expected = "void f(void)\n{\n    if( z )\n    {\n        if( a )\n        {\n            x = 1;\n        }\n        else\n#ifdef X\n            if( b )\n            {\n                x = 2;\n            }\n            else\n#endif\n                if( c )\n                {\n                    x = 3;\n                }\n    }\n    else if( d )\n    {\n        y = 1;\n    }\n}\n";
    check(input, &["--style=allman"], expected);
    check(expected, &["--style=allman"], expected);
}

#[test]
fn run_in_brace_of_a_split_else_body_aligns_to_its_header() {
    let input = "void f(void)\n{\n  if(a) {\n    x = 1;\n  }\n  else\n\n  if(c) {\n    y = 1;\n  }\n  z = 2;\n}\n";
    let expected = "void f(void)\n{   if(a)\n    {   x = 1; }\n    else\n\n        if(c)\n        {   y = 1; }\n    z = 2; }\n";
    check(input, &["--style=pico"], expected);
    check(expected, &["--style=pico"], expected);
}

#[test]
fn split_else_body_with_a_multiline_condition_closes_at_its_header() {
    let input = "void f(void)\n{\n  if(a) {\n    x = 1;\n  }\n  else\n\n  if(c &&\n     d) {\n    y = 1;\n  }\n  z = 2;\n}\n";
    let expected = "void f(void)\n{\n    if(a) {\n        x = 1;\n    } else\n\n        if(c &&\n                d) {\n            y = 1;\n        }\n    z = 2;\n}\n";
    check(input, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

#[test]
fn declaration_after_an_attribute_macro_line_takes_the_statement_indent() {
    let input = "CURL_EXTERN ALLOC_FUNC ALLOC_SIZE2(1, 2)\n  void *curl_dbg_calloc(size_t wanted_elements, size_t wanted_size,\n                        int line, const char *source);\n";
    let expected = "CURL_EXTERN ALLOC_FUNC ALLOC_SIZE2(1, 2)\nvoid *curl_dbg_calloc(size_t wanted_elements, size_t wanted_size,\n                      int line, const char *source);\n";
    let options = ["--break-after-logical", "--min-conditional-indent=0"];
    check(input, &options, expected);
    check(expected, &options, expected);
}

#[test]
fn return_type_stays_split_when_a_directive_splits_the_parameters() {
    let source = "static int\nf(int a,\n#if X\n  int b,\n#endif\n  int c);\n";
    check(source, &["--attach-return-type-decl"], source);
}

#[test]
fn max_code_length_splits_at_the_last_bitwise_operator_not_after_a_unary_one() {
    let input = "void f(void)\n{\n    mask = (a - 1) | ~(DV_I_48_0_bit | DV_I_51_0_bit | DV_I_52_0_bit | DV_II_45_0_bit | DV_II_46_0_bit);\n    ok = (a - 1) && !(DV_I_48_0_bit || DV_I_51_0_bit || DV_I_52_0_bit || DV_II_45_0_bit || DV);\n}\n";
    let expected = "void f(void)\n{\n    mask = (a - 1) | ~(DV_I_48_0_bit | DV_I_51_0_bit | DV_I_52_0_bit |\n                       DV_II_45_0_bit | DV_II_46_0_bit);\n    ok = (a - 1) && !(DV_I_48_0_bit || DV_I_51_0_bit || DV_I_52_0_bit\n                      || DV_II_45_0_bit || DV);\n}\n";
    check(input, &["--max-code-length=80"], expected);
    check(expected, &["--max-code-length=80"], expected);
}

#[test]
fn subscript_row_aligns_past_the_open_bracket() {
    let input = "void f(void)\n{\n    buf[k] = special[my_random() %\n        ARRAY_SIZE(special)];\n    x = a[b() %\n        c];\n}\n";
    let expected = "void f(void)\n{\n    buf[k] = special[my_random() %\n                     ARRAY_SIZE(special)];\n    x = a[b() %\n          c];\n}\n";
    check(input, &[], expected);
    check(expected, &[], expected);
}

#[test]
fn comment_after_a_case_block_takes_the_label_level_only_before_a_label() {
    let input = "int f(int op)\n{\n  switch( op ){\n    case 1: {\n      x = 1;\n      break;\n    }\n\n/* c1\n** c2 */\n#if A\n    case 2:\n      break;\n#endif\n    }\n    switch( op ){\n    case 1: {\n      break;\n    }\n/* c3 */\n/* c4 */\n    case 2:\n      break;\n  }\n}\n";
    let expected = "int f(int op)\n{\n    switch( op ) {\n    case 1: {\n        x = 1;\n        break;\n    }\n\n        /* c1\n        ** c2 */\n#if A\n    case 2:\n        break;\n#endif\n    }\n    switch( op ) {\n    case 1: {\n        break;\n    }\n    /* c3 */\n    /* c4 */\n    case 2:\n        break;\n    }\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn comment_trailing_a_switch_brace_stands_at_the_case_labels() {
    let input = "int f(int c)\n{\n  switch( c ){  /* a\n          ** b */\n    case 1:\n      return 1;\n  }\n}\n";
    let expected = "int f(int c)\n{\n    switch( c ) {\n    /* a\n            ** b */\n    case 1:\n        return 1;\n    }\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn vtk_initializer_brace_after_a_local_struct_stands_at_its_close() {
    let input = "static int f(void){\n  static const T aMult[] = {\n    { \"KiB\", 1024 },\n  };\n  struct { int a; } b = {\n    1\n  };\n  return 0;\n}\nstruct { int a; } b[] = {\n    {1},\n};\n";
    let expected = "static int f(void)\n{\n    static const T aMult[] =\n        {\n            { \"KiB\", 1024 },\n        };\n    struct\n        {\n        int a;\n        } b =\n        {\n        1\n        };\n    return 0;\n}\nstruct\n{\n    int a;\n} b[] =\n{\n    {1},\n};\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
}

#[test]
fn vtk_breaks_multiline_rows_of_file_scope_arrays_a_level_in() {
    let input = "static ngx_command_t  cmds[] = {\n\n    { ngx_string(\"slice\"),\n      NGX_HTTP_MAIN_CONF|NGX_CONF_TAKE1,\n      ngx_conf_set_size_slot,\n      0,\n      NULL },\n\n      ngx_null_command\n};\n\nstatic T x[] = {\n    { a, b },\n    { c,\n      d },\n};\n";
    let expected = "static ngx_command_t  cmds[] = {\n\n        {\n        ngx_string(\"slice\"),\n        NGX_HTTP_MAIN_CONF|NGX_CONF_TAKE1,\n        ngx_conf_set_size_slot,\n        0,\n        NULL\n        },\n\n    ngx_null_command\n};\n\nstatic T x[] =\n{\n    { a, b },\n        {\n        c,\n        d\n        },\n};\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
}

#[test]
fn whitesmith_splits_every_label_of_the_first_case_line() {
    let input =
        "void f(int c)\n{\n    switch (c) {\n    case 1: case 2: case 3:\n        g();\n    }\n}\n";
    let expected = "void f(int c)\n    {\n    switch (c)\n        {\n        case 1:\n        case 2:\n        case 3:\n            g();\n        }\n    }\n";
    check(input, &["--style=whitesmith"], expected);
    check(expected, &["--style=whitesmith"], expected);
}

#[test]
fn split_statement_keeps_an_adjacent_line_comment() {
    let input = "void f(void)\n{\n    a=0; b=1;// c\n    a=0; if (x) b=1;// e\n}\n";
    let expected = "void f(void)\n{\n    a=0;\n    b=1;// c\n    a=0;\n    if (x) b=1;// e\n}\n";
    check(input, &[], expected);
    check(expected, &[], expected);
}

#[test]
fn comment_after_a_broken_else_brace_keeps_its_source_gap() {
    let input = "void f(void)\n{\n  if (a) {\n    x = 1;\n  }\n  else { // is finite\n    y = 2;\n  }\n  if (b) {\n    x = 1;\n  } else {   // c2\n    y = 2;\n  }\n}\n";
    let expected = "void f(void)\n{\n    if (a)\n        {\n            x = 1;\n        }\n    else   // is finite\n        {\n            y = 2;\n        }\n    if (b)\n        {\n            x = 1;\n        }\n    else       // c2\n        {\n            y = 2;\n        }\n}\n";
    check(input, &["--style=gnu"], expected);
    check(expected, &["--style=gnu"], expected);
}

#[test]
fn one_line_enum_body_on_its_own_line_stays_there() {
    let input = "enum\n{ OPT_A, OPT_B };\nenum e\n{ A, B };\nint f(void)\n{\n    enum\n    { C, D };\n    int i;\n}\n";
    for (style, expected) in [
        ("--style=kr", input),
        (
            "--style=whitesmith",
            "enum\n    { OPT_A, OPT_B };\nenum e\n    { A, B };\nint f(void)\n    {\n    enum\n        { C, D };\n    int i;\n    }\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

#[test]
fn add_braces_wraps_a_body_that_starts_with_a_paren() {
    let input = "void f(void){\n  if( id ) (void)g(id, op);\n  if( id ) (void)x;\n  if( id ) g(id);\n  if (a) (x)++;\n}\n";
    let expected = "void f(void) {\n    if( id ) {\n        (void)g(id, op);\n    }\n    if( id ) {\n        (void)x;\n    }\n    if( id ) {\n        g(id);\n    }\n    if (a) {\n        (x)++;\n    }\n}\n";
    check(input, &["--add-braces"], expected);
    check(expected, &["--add-braces"], expected);
}

#[test]
fn deref_statement_is_no_comment_row_before_an_else() {
    let input = "void g(void) {\n  if (a) return;\n  else if (b)  /* c */\n    *l1 = l2;\n  else {\n    y = 2;\n  }\n}\n";
    let expected = "void g(void)\n{\n    if (a) {\n        return;\n    } else if (b) { /* c */\n        *l1 = l2;\n    } else {\n        y = 2;\n    }\n}\n";
    check(input, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

#[test]
fn comment_against_an_opening_brace_stays_against_it() {
    let input = "void f(void)\n{\n    if (a) {/* c1 */\n        x();\n    } else {/* c2 */\n        y();\n    }\n    if (b) {// c3\n        x();\n    }\n}\n";
    let expected = "void f(void)\n{\n    if (a) {/* c1 */\n        x();\n    } else {/* c2 */\n        y();\n    }\n    if (b) {// c3\n        x();\n    }\n}\n";
    check(input, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

#[test]
fn added_brace_takes_two_columns_from_a_tab_gap() {
    let input = "void f(void)\n{\n\tif (a)\n\t\tx();\n\telse if (!len)\t\t/* c */\n\t\tgoto out;\n\tif (!len) \t/* d */\n\t\tgoto out;\n}\n";
    let expected = "void f(void)\n{\n    if (a) {\n        x();\n    } else if (!len) {\t/* c */\n        goto out;\n    }\n    if (!len) {\t/* d */\n        goto out;\n    }\n}\n";
    check(input, &["--style=1tbs"], expected);
}

#[test]
fn comment_after_a_line_leading_function_brace_stays_below_it() {
    let input = "void f(void)\n{ // c\n    x();\n}\nvoid g(void)\n{/* d */\n    x();\n}\n";
    let expected =
        "void f(void)\n{\n    // c\n    x();\n}\nvoid g(void)\n{\n    /* d */\n    x();\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn bare_block_moves_a_line_comment_into_its_body() {
    let input = "void f(void)\n{\n    x();\n    {/* emcc */\n        y();\n    }\n    {// c\n        z();\n    }\n}\n";
    let expected = "void f(void)\n{\n    x();\n    {/* emcc */\n        y();\n    }\n    {\n        // c\n        z();\n    }\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn run_in_enum_after_a_typedef_enum_closes_at_its_own_brace() {
    let input = "typedef enum { C = 0,\n D = 1 } s;\nenum e { A,\n  B };\n";
    for (style, expected) in [
        (
            "--style=kr",
            "typedef enum { C = 0,\n               D = 1\n             } s;\nenum e { A,\n         B\n       };\n",
        ),
        (
            "--style=mozilla",
            "typedef enum\n{ C = 0,\n  D = 1\n} s;\nenum e\n{ A,\n  B\n};\n",
        ),
    ] {
        check(input, &[style], expected);
        if style == "--style=kr" {
            check(expected, &[style], expected);
        }
    }
}

#[test]
fn struct_head_split_over_lines_still_opens_a_struct() {
    let input = "struct\nBCinfo { int dp0, dp1; };\nstruct BC2 { int a; };\n";
    let expected = "struct\n    BCinfo {\n    int dp0, dp1;\n};\nstruct BC2 {\n    int a;\n};\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn comment_gap_after_a_brace_follows_the_closing_brace_placement() {
    let input = "void f(void)\n{\n  if( a ){       /* c */\n    y();\n  }\n  if( a )   {       /* d */\n    y();\n  }\n  if(a){   /* e */\n    y();\n  }else{     /* f */\n    z();\n  }\n}\n";
    for (style, expected) in [
        (
            "--style=stroustrup",
            "void f(void)\n{\n    if( a ) {      /* c */\n        y();\n    }\n    if( a )   {       /* d */\n        y();\n    }\n    if(a) {  /* e */\n        y();\n    }\n    else {     /* f */\n        z();\n    }\n}\n",
        ),
        (
            "--style=kr",
            "void f(void)\n{\n    if( a ) {      /* c */\n        y();\n    }\n    if( a )   {       /* d */\n        y();\n    }\n    if(a) {  /* e */\n        y();\n    } else {    /* f */\n        z();\n    }\n}\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

#[test]
fn max_code_length_prefers_a_comma_to_a_comparison() {
    let input = "void f(void)\n{\n    if (RedisModule_CreateCommand(ctx, \"getkeys.command\", getkeys_command, \"getkeys-api\", 0, 0, 0) == REDISMODULE_ERR) {\n        return 1;\n    }\n    x = aaaaaaaaaaaaaaa(bbbbbbbbbbbbb, cccccccccccccc, ddddddddddddddd, eeeeeeeeeeeee, fffff) == ggggggggggggggggggg;\n    y = aaaaaaaaaaaaaaa(bbbbbbbbbbbbb, cccccccccccccc, ddddddddddddddd, eeeeeeeeeeeee, ffffffff) + ggggggggggggggggggg;\n}\n";
    let expected = "void f(void)\n{\n    if (RedisModule_CreateCommand(ctx, \"getkeys.command\", getkeys_command, \"getkeys-api\", 0, 0,\n                                  0) == REDISMODULE_ERR) {\n        return 1;\n    }\n    x = aaaaaaaaaaaaaaa(bbbbbbbbbbbbb, cccccccccccccc, ddddddddddddddd, eeeeeeeeeeeee,\n                        fffff) == ggggggggggggggggggg;\n    y = aaaaaaaaaaaaaaa(bbbbbbbbbbbbb, cccccccccccccc, ddddddddddddddd, eeeeeeeeeeeee,\n                        ffffffff) + ggggggggggggggggggg;\n}\n";
    check(input, &["--max-code-length=100"], expected);
    check(expected, &["--max-code-length=100"], expected);
}

#[test]
fn added_brace_ignores_trailing_source_whitespace() {
    let input = "void f(void)\n{\n    while (*link != NULL) {\n            if (key == visitedKey || cmpFunc( &cmpCache, key, visitedKey))                \n                return link;\n    }\n}\n";
    let expected = "void f(void)\n{\n    while (*link != NULL) {\n        if (key == visitedKey || cmpFunc( &cmpCache, key, visitedKey)) {\n            return link;\n        }\n    }\n}\n";
    check(input, &["--style=1tbs"], expected);
}

#[test]
fn parenthesized_cast_operand_is_no_cast_type() {
    let input = "long f(void)\n{\n    return (((long long)tv.tv_sec)*1000) + (tv.tv_usec/1000);\n    x = (((int)tv)*1000);\n}\n";
    let expected = "long f(void)\n{\n    return (((long long)tv.tv_sec) * 1000) + (tv.tv_usec / 1000);\n    x = (((int)tv) * 1000);\n}\n";
    check(input, &["--pad-oper"], expected);
    check(expected, &["--pad-oper"], expected);
}

#[test]
fn star_before_a_number_after_a_macro_call_multiplies() {
    let source = "int f(void)\n{\n    return loose_count > (DIV_ROUND_UP(((unsigned long) limit), 256) * 256);\n}\n";
    check(source, &["--pad-oper"], source);
}

#[test]
fn reindented_comment_keeps_blank_lines_empty() {
    let input = "switch(state){\n            /**\n               JS regexes\n\n      case S_C: /* C comment */\n}\n";
    let expected =
        "switch(state)\n{\n    /**\n       JS regexes\n\n    case S_C: /* C comment */\n}\n";
    check(input, &["--style=stroustrup"], expected);
}

#[test]
fn empty_line_after_trailing_block_comment_stays_empty() {
    let input = "void f()\n{\n\t/* assume\n\t * is.\n\t */\n\n\tx();\n\t/* one */\n\n\tw();\n\tx = 1; /* a\n\t */ y = 2;\n\n\tz();\n}\n";
    let expected = "void f()\n{\n    /* assume\n     * is.\n     */\n    \n    x();\n    /* one */\n    \n    w();\n    x = 1; /* a\n\t */ y = 2;\n\n    z();\n}\n";
    check(input, &["--style=allman", "--fill-empty-lines"], expected);
}

#[test]
fn switch_labels_on_one_line_share_a_whitesmith_case_block() {
    let input = "void f() {\n    switch (*s) {\n      case 1: case 2: case 3: {\n        x();\n        break;\n      }\n      default: y();\n    }\n}\n";
    let expected = "void f()\n    {\n    switch (*s)\n        {\n        case 1:\n        case 2:\n        case 3:\n            {\n            x();\n            break;\n            }\n        default:\n            y();\n        }\n    }\n";
    check(input, &["--style=whitesmith"], expected);
    check(expected, &["--style=whitesmith"], expected);
}

#[test]
fn vtk_nested_switch_case_block_brace_follows_its_label() {
    let input = "void f() {\n  switch (*p) {\n    case 2: {\n      switch (x) {\n        case 3: {\n          s = 1;\n        }\n      }\n    }\n  }\n}\n";
    let expected = "void f()\n{\n    switch (*p)\n        {\n        case 2:\n            {\n            switch (x)\n                {\n                case 3:\n                    {\n                    s = 1;\n                    }\n                }\n            }\n        }\n}\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
}

#[test]
fn statement_label_after_default_owns_whitesmith_block() {
    let input = "void f() {\n  switch (*p) {\n    default: dflt: {\n      switch (*ep) {\n        case 2: {\n          y();\n        }\n      }\n    }\n  }\n}\n";
    let expected = "void f()\n    {\n    switch (*p)\n        {\n        default:\ndflt:\n                {\n                switch (*ep)\n                    {\n                    case 2:\n                        {\n                        y();\n                        }\n                    }\n                }\n        }\n    }\n";
    check(input, &["--style=whitesmith"], expected);
    check(expected, &["--style=whitesmith"], expected);
}

#[test]
fn statement_label_after_default_owns_allman_block() {
    let input = "void f() {\n  switch (*p) {\n    default: dflt: {\n      switch (*ep) {\n        case 2: {\n          y();\n        }\n      }\n    }\n  }\n}\n";
    let expected = "void f()\n{\n    switch (*p)\n    {\n    default:\ndflt:\n        {\n            switch (*ep)\n            {\n            case 2:\n            {\n                y();\n            }\n            }\n        }\n    }\n}\n";
    check(input, &["--style=allman"], expected);
    check(expected, &["--style=allman"], expected);
}

#[test]
fn leading_assignment_ignores_continuation_indent() {
    let input = "void f()\n{\n    int x\n      = 3;\n    int y =\n      3;\n    x = a\n      + b;\n    foo(a,\n        b)\n      + c;\n}\n";
    let expected = "void f()\n{\n    int x\n        = 3;\n    int y =\n                3;\n    x = a\n        + b;\n    foo(a,\n        b)\n    + c;\n}\n";
    check(input, &["--indent-continuation=3"], expected);
    check(expected, &["--indent-continuation=3"], expected);
}

#[test]
fn max_code_length_splits_before_a_prefix_operator_not_after_it() {
    let input = "void f()\n{\n    int *n = &fsync_component_names_and_some_more_text[index];\n    cfg->precomposed_unicode = -1; /* see probe_utf8_pathname_composition() */\n    cfg->precomposed_unicode = 1; // see probe_utf8_pathname_composition()\n}\n";
    let expected = "void f()\n{\n    int *n = &fsync_component_names_and_some_more_text[index];\n    cfg->precomposed_unicode =\n        -1; /* see probe_utf8_pathname_composition() */\n    cfg->precomposed_unicode =\n        1; // see probe_utf8_pathname_composition()\n}\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn max_code_length_splits_lines_holding_a_string_call() {
    let input = "void f()\n{\n    warning(_(\"ignoring unknown core.fsync %s\"), component, another_arg);\n    warning(component, another_arg, _(\"ignoring unknown core.fsync %s\"));\n    warning(component, another_arg, other_component, _(\"ignoring %s\"));\n    xxxxxxx = component + another_arg + other_component + _(\"ignoring %s\");\n    warning(_(\"ignoring unknown core.fsync component and more %s\"), c);\n    warning(_(\"ignoring unknown\"), component, another_arg, more_args, x);\n}\n";
    let expected = "void f()\n{\n    warning(_(\"ignoring unknown core.fsync %s\"), component,\n            another_arg);\n    warning(component, another_arg,\n            _(\"ignoring unknown core.fsync %s\"));\n    warning(component, another_arg, other_component,\n            _(\"ignoring %s\"));\n    xxxxxxx = component + another_arg + other_component +\n              _(\"ignoring %s\");\n    warning(_(\"ignoring unknown core.fsync component and more %s\"),\n            c);\n    warning(_(\"ignoring unknown\"), component, another_arg,\n            more_args, x);\n}\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn vtk_added_one_line_brace_indents_within_a_nested_block() {
    let input = "void f()\n{\n\tif (m) {\n\t\tclear(repo);\n\t\tif (!w)\n\t\t\tclear2(repo);\n\t}\n\twhile (x) {\n\t\tif (!w)\n\t\t\tcontinue;\n\t}\n}\n";
    let expected = "void f()\n{\n    if (m)\n        {\n        clear(repo);\n        if (!w)\n            { clear2(repo); }\n        }\n    while (x)\n        {\n        if (!w)\n            { continue; }\n        }\n}\n";
    check(input, &["--style=vtk", "--add-one-line-braces"], expected);
    check(
        expected,
        &["--style=vtk", "--add-one-line-braces"],
        expected,
    );
}

#[test]
fn whitesmith_statement_after_nested_added_one_line_block() {
    let input = "void f()\n{\n\twhile (x) {\n\t\tfor (i = 0; i < n; i++)\n\t\t\tif (!w)\n\t\t\t\tbreak;\n\n\t\ty();\n\t}\n\tfor (i = 0; i < n; i++)\n\t\tif (!w)\n\t\t\tbreak;\n\tz();\n}\n";
    let expected = "void f()\n    {\n    while (x)\n        {\n        for (i = 0; i < n; i++)\n            if (!w)\n                { break; }\n\n        y();\n        }\n    for (i = 0; i < n; i++)\n        if (!w)\n            { break; }\n    z();\n    }\n";
    check(
        input,
        &["--style=whitesmith", "--add-one-line-braces"],
        expected,
    );
    check(
        expected,
        &["--style=whitesmith", "--add-one-line-braces"],
        expected,
    );
}

#[test]
fn vtk_statement_after_nested_added_one_line_block() {
    let input = "void f()\n{\n\twhile (x) {\n\t\tfor (i = 0; i < n; i++)\n\t\t\tif (!w)\n\t\t\t\tbreak;\n\n\t\ty();\n\t}\n\tfor (i = 0; i < n; i++)\n\t\tif (!w)\n\t\t\tbreak;\n\tz();\n}\n";
    let expected = "void f()\n{\n    while (x)\n        {\n        for (i = 0; i < n; i++)\n            if (!w)\n                { break; }\n\n        y();\n        }\n    for (i = 0; i < n; i++)\n        if (!w)\n        { break; }\n    z();\n}\n";
    check(input, &["--style=vtk", "--add-one-line-braces"], expected);
    check(
        expected,
        &["--style=vtk", "--add-one-line-braces"],
        expected,
    );
}

#[test]
fn break_blocks_keeps_an_empty_block_closed_up_to_its_closing_header() {
    let input = "void f()\n{\n\tif (a) {\n\t\tb();\n\t} else if (x) {\n\t\ty();\n\t}\n\tif (a) {\n\t} else if (x) {\n\t\ty();\n\t}\n\tif (a) {\n\t\t;\n\t} else {\n\t\ty();\n\t}\n}\n";
    let expected = "void f()\n{\n    if (a)\n    {\n        b();\n    }\n\n    else if (x)\n    {\n        y();\n    }\n\n    if (a)\n    {\n    }\n    else if (x)\n    {\n        y();\n    }\n\n    if (a)\n    {\n        ;\n    }\n\n    else\n    {\n        y();\n    }\n}\n";
    check(input, &["--style=allman", "--break-blocks=all"], expected);
    check(
        expected,
        &["--style=allman", "--break-blocks=all"],
        expected,
    );
}

#[test]
fn attached_break_blocks_keeps_an_empty_block_closed_up() {
    let input = "void f()\n{\n\tif (a) {\n\t\tb();\n\t} else if (x) {\n\t\ty();\n\t}\n\tif (a) {\n\t} else if (x) {\n\t\ty();\n\t}\n\tif (a) {\n\t\t;\n\t} else {\n\t\ty();\n\t}\n}\n";
    let expected = "void f()\n{\n    if (a) {\n        b();\n\n    } else if (x) {\n        y();\n    }\n\n    if (a) {\n    } else if (x) {\n        y();\n    }\n\n    if (a) {\n        ;\n\n    } else {\n        y();\n    }\n}\n";
    check(input, &["--style=kr", "--break-blocks=all"], expected);
    check(expected, &["--style=kr", "--break-blocks=all"], expected);
}

#[test]
fn lisp_keeps_switch_labels_on_one_line_with_their_block() {
    let input = "void f() {\n    switch (*s) {\n      case 1: case 2: case 3: {\n        x();\n        break;\n      }\n      default: y();\n    }\n}\n";
    let expected = "void f() {\n    switch (*s) {\n    case 1: case 2: case 3: {\n        x();\n        break; }\n    default: y(); } }\n";
    check(input, &["--style=lisp"], expected);
    check(expected, &["--style=lisp"], expected);
}

#[test]
fn indented_conditional_continuation_takes_tabs_in_a_block() {
    let input = "#include \"a.h\"\n\n#if !defined(A) || \\\n  !defined(B)\n#define X\n#endif\nint x;\nvoid f()\n{\n#if !defined(A) || \\\n  !defined(B)\n    x();\n#endif\n}\n";
    let expected = "#include \"a.h\"\n\n#if !defined(A) || \\\n\t!defined(B)\n\t#define X\n#endif\nint x;\nvoid f()\n{\n#if !defined(A) || \\\n  !defined(B)\n\tx();\n#endif\n}\n";
    check(
        input,
        &["--indent=tab=4", "--indent-preproc-block"],
        expected,
    );
    check(
        expected,
        &["--indent=tab=4", "--indent-preproc-block"],
        expected,
    );
}

#[test]
fn indented_conditional_continuation_takes_tabs_in_code() {
    let input = "#include \"a.h\"\n\n#if !defined(A) || \\\n  !defined(B)\n#define X\n#endif\nint x;\nvoid f()\n{\n#if !defined(A) || \\\n  !defined(B)\n    x();\n#endif\n}\n";
    let expected = "#include \"a.h\"\n\n#if !defined(A) || \\\n!defined(B)\n#define X\n#endif\nint x;\nvoid f()\n{\n\t#if !defined(A) || \\\n\t!defined(B)\n\tx();\n\t#endif\n}\n";
    check(
        input,
        &["--indent=tab=4", "--indent-preproc-cond"],
        expected,
    );
    check(
        expected,
        &["--indent=tab=4", "--indent-preproc-cond"],
        expected,
    );
}

#[test]
fn force_tab_comment_rows_keep_their_opener_tabs() {
    let input = "struct s {\n\t/*\n\t * Next\n\t *     more\n\t */\n\tint x;\n};\nvoid f()\n{\n        /* a\n               b */\n\tif (a) {\n\t\t/* a comes\n\t\tif (b)\n\t\t\treturn 1;\n\t\t*/\n\t}\n}\n";
    let expected = "struct s {\n\t/*\n\t    Next\n\t       more\n\t*/\n\tint x;\n};\nvoid f()\n{\n\t/*  a\n\t       b */\n\tif (a) {\n\t\t/*  a comes\n\t\t    if (b)\n\t\t\treturn 1;\n\t\t*/\n\t}\n}\n";
    check(
        input,
        &["--indent=force-tab=4", "--remove-comment-prefix"],
        expected,
    );
}

#[test]
fn force_tab_comment_rows_keep_source_tabs() {
    let input = "struct s {\n\t/*\n\t * Next\n\t *     more\n\t */\n\tint x;\n};\nvoid f()\n{\n        /* a\n               b */\n\tif (a) {\n\t\t/* a comes\n\t\tif (b)\n\t\t\treturn 1;\n\t\t*/\n\t}\n}\n";
    let expected = "struct s {\n\t/*\n\t * Next\n\t *     more\n\t */\n\tint x;\n};\nvoid f()\n{\n\t/* a\n\t       b */\n\tif (a) {\n\t\t/* a comes\n\t\tif (b)\n\t\t\treturn 1;\n\t\t*/\n\t}\n}\n";
    check(input, &["--indent=force-tab=8"], expected);
}

#[test]
fn gnu_conditional_directive_in_broken_else_if_block_follows_the_code() {
    let input = "void f()\n{\n  if( a ){\n    x();\n  }else if( v==0 ){\n    y();\n  }else{\n    z();\n#ifdef H\n    w();\n#endif\n  }\n}\n";
    let expected = "void f()\n{\n    if( a )\n        {\n            x();\n        }\n    else\n        if( v==0 )\n            {\n                y();\n            }\n        else\n            {\n                z();\n                #ifdef H\n                w();\n                #endif\n            }\n}\n";
    check(
        input,
        &["--style=gnu", "--indent-preproc-cond", "--break-elseifs"],
        expected,
    );
    check(
        expected,
        &["--style=gnu", "--indent-preproc-cond", "--break-elseifs"],
        expected,
    );
}

#[test]
fn attached_conditional_directive_in_broken_else_if_block_follows_the_code() {
    let input = "void f()\n{\n  if( a ){\n    x();\n  }else if( v==0 ){\n    y();\n  }else{\n    z();\n#ifdef H\n    w();\n#endif\n  }\n}\n";
    let expected = "void f()\n{\n    if( a ) {\n        x();\n    } else\n        if( v==0 ) {\n            y();\n        } else {\n            z();\n            #ifdef H\n            w();\n            #endif\n        }\n}\n";
    check(
        input,
        &["--style=kr", "--indent-preproc-cond", "--break-elseifs"],
        expected,
    );
    check(
        expected,
        &["--style=kr", "--indent-preproc-cond", "--break-elseifs"],
        expected,
    );
}

#[test]
fn enum_member_with_parenthesized_comment_takes_the_member_indent() {
    let input =
        "typedef enum {\n  A, /* First (state) */\n  B, /* (Possibly) First */\n  C,\n} x;\n";
    let expected =
        "typedef enum {\n    A, /* First (state) */\n    B, /* (Possibly) First */\n    C,\n} x;\n";
    check(input, &["--min-conditional-indent=0"], expected);
    check(expected, &["--min-conditional-indent=0"], expected);
}

#[test]
fn max_code_length_paren_at_line_end_stacks_one_continuation() {
    let input = "void f()\n{\n    foo(aaaaaaa, bar(cccccccccccccccccccccccccc, ddddddddddddddddddddddd));\n    x = foo(aaaaaaa, bar(cccccccccccccccccccccccccc, ddddddddddddddddddddddd));\n    xxxxxxxxxxxxxxxxxxxxxxxxx(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);\n    xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx(bbbbbbbbbbbbbbbbbbbbbbbbbb);\n    yy = xxxxxxxxxxxxxxxxxxxxxxxxx(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);\n    return xxxxxxxxxxxxxxxxxxxxxxxxx(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);\n    if (xxxxxxxxxxxxxxxxxxxxxxxxx(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb))\n        x();\n}\n";
    let expected = "void f()\n{\n    foo(aaaaaaa, bar(cccccccccccccccccccccccccc,\n                     ddddddddddddddddddddddd));\n    x = foo(aaaaaaa, bar(cccccccccccccccccccccccccc,\n                         ddddddddddddddddddddddd));\n    xxxxxxxxxxxxxxxxxxxxxxxxx(\n        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);\n    xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx(\n        bbbbbbbbbbbbbbbbbbbbbbbbbb);\n    yy = xxxxxxxxxxxxxxxxxxxxxxxxx(\n             bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);\n    return xxxxxxxxxxxxxxxxxxxxxxxxx(\n               bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);\n    if (xxxxxxxxxxxxxxxxxxxxxxxxx(\n                bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb))\n        x();\n}\n";
    check(input, &["--max-code-length=50"], expected);
    check(expected, &["--max-code-length=50"], expected);
}

#[test]
fn max_code_length_moves_a_string_call_after_a_call_paren() {
    let input = "void f()\n{\n    return jv_invalid_with_msg(jv_string(\"jq was compiled without ONIGURUMA regex library.\"));\n    if (jv_invalid_with_msg(jv_string(\"jq was compiled without ONIGURUMA regex library.\")))\n        x();\n    while(jv_invalid_with_msg(jv_string(\"jq was compiled without ONIGURUMA regex library.\")))\n        x();\n    xxxxxxxxxxxxx(jv_string(\"jq was compiled without ONIGURUMA regex library. and more\"));\n}\n";
    let expected = "void f()\n{\n    return jv_invalid_with_msg(\n               jv_string(\"jq was compiled without ONIGURUMA regex library.\"));\n    if (jv_invalid_with_msg(\n                jv_string(\"jq was compiled without ONIGURUMA regex library.\")))\n        x();\n    while(jv_invalid_with_msg(\n                jv_string(\"jq was compiled without ONIGURUMA regex library.\")))\n        x();\n    xxxxxxxxxxxxx(\n        jv_string(\"jq was compiled without ONIGURUMA regex library. and more\"));\n}\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn cast_dereference_in_brackets_before_assignment_keeps_its_spacing() {
    let input = "void f()\n{\n    while (*p) {\n        lookup[(int)*p] = p + 1;\n        x = (int)*p;\n        y = a[(int)*p];\n    }\n}\n";
    let expected = "void f()\n{\n    while (*p) {\n        lookup[(int)*p] = p + 1;\n        x = (int) * p;\n        y = a[(int) * p];\n    }\n}\n";
    check(input, &["--pad-oper"], expected);
    check(expected, &["--pad-oper"], expected);
}

#[test]
fn max_code_length_keeps_a_block_brace_with_its_head() {
    let input = "void f()\n{\n    TEST(\"Verify that a rehashing dict node in the list is ok\") {\n        x();\n    }\n    TESTX(abcdefghijklmnopqrstuvwxyz, abcdefghijklmnopqrstuvwxyz) {\n        x();\n    }\n}\n";
    let expected = "void f()\n{\n    TEST(\"Verify that a rehashing dict node in the list is ok\") {\n        x();\n    }\n    TESTX(abcdefghijklmnopqrstuvwxyz,\n          abcdefghijklmnopqrstuvwxyz) {\n        x();\n    }\n}\n";
    check(input, &["--max-code-length=60"], expected);
    check(expected, &["--max-code-length=60"], expected);
}

#[test]
fn struct_array_rows_in_a_case_block_take_the_row_indent() {
    let input = "void f()\n{\n  switch (x) {\n    case 11: {    /* object_config */\n      struct ObjConfOpt {\n        const char *zName;\n        int opt;\n      } aOpt[] = {\n        { \"size\", 1 },\n        { 0, 0 }\n      };\n      break;\n    }\n  }\n}\n";
    let expected = "void f()\n{\n    switch (x) {\n        case 11: {    /* object_config */\n            struct ObjConfOpt {\n                const char *zName;\n                int opt;\n            } aOpt[] = {\n                { \"size\", 1 },\n                { 0, 0 }\n            };\n            break;\n        }\n    }\n}\n";
    check(
        input,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
    check(
        expected,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
}

#[test]
fn padding_before_an_ending_brace_shrinks_its_comment_gap() {
    let input = "void f()\n{\n    if(a<b) {  /* c */\n        x();\n    }\n    if(a<b)   /* c */\n        x();\n    x=a<b;  /* c */\n    if(carry) {         /* first */\n        x();\n    }\n}\n";
    let expected = "void f()\n{\n    if (a < b) { /* c */\n        x();\n    }\n    if (a < b) /* c */\n        x();\n    x = a < b; /* c */\n    if (carry) {        /* first */\n        x();\n    }\n}\n";
    check(input, &["--pad-oper", "--pad-header"], expected);
    check(expected, &["--pad-oper", "--pad-header"], expected);
}

#[test]
fn define_assignment_on_the_directive_line_aligns_its_continuation() {
    let input = "#define SWAPINIT(a, es) swaptype = (uintptr_t)a % sizeof(long) || \\\n\tes % sizeof(long) ? 2 : es == sizeof(long)? 0 : 1;\n#define X(a) y = a + \\\n\tb;\n#define Z(a) foo(a, \\\n\tb);\n";
    let expected = "#define SWAPINIT(a, es) swaptype = (uintptr_t)a % sizeof(long) || \\\n                                   es % sizeof(long) ? 2 : es == sizeof(long)? 0 : 1;\n#define X(a) y = a + \\\n                 b;\n#define Z(a) foo(a, \\\n                 b);\n";
    check(input, &["--indent-preproc-define"], expected);
    check(expected, &["--indent-preproc-define"], expected);
}

#[test]
fn added_braces_around_a_dereferenced_cast_do_not_panic() {
    let input = "void f()\n{\n    if( n!=P4_VTAB ) freeP4(db, n, (void*)*(char**)&zP4);\n}\n";
    let expected = "void f()\n{\n    if( n != P4_VTAB ) {\n        freeP4(db, n, (void*) * (char**)&zP4);\n    }\n}\n";
    check(input, &["--add-braces", "--pad-oper"], expected);
    check(expected, &["--add-braces", "--pad-oper"], expected);
}

#[test]
fn max_code_length_splits_a_function_pointer_declarator_without_panicking() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--max-code-length=60".to_owned()])
        .expect("valid options");
    let input = "ZLIB_INTERNAL unsigned long (*crc32_z_hook)(unsigned long crc, const unsigned char FAR *buf, z_size_t len) = crc32_z;\n";
    let output = format_bytes(input.as_bytes(), &options).expect("format bytes");
    let output = String::from_utf8(output).expect("utf8");
    assert_eq!(non_whitespace(&output), non_whitespace(input));
}

#[test]
fn comment_led_statement_in_a_default_block_takes_the_block_column() {
    let input = "void f()\n{\n  switch( e ){\n    default: {\n      e = 1;\n      /* no break */ deliberate_fall_through\n    }\n    case 2:\n      x();\n  }\n}\n";
    let expected = "void f()\n{\n    switch( e ) {\n        default: {\n            e = 1;\n            /* no break */ deliberate_fall_through\n        }\n        case 2:\n            x();\n    }\n}\n";
    check(input, &["--indent-switches"], expected);
    check(expected, &["--indent-switches"], expected);
}

#[test]
fn unpadding_keeps_an_attached_trailing_comment_at_its_column() {
    let input = "void f()\n{\n    if (a) {\n        assert( (pPg->flags & PGHDR_DIRTY)==0 );/* Cannot be both CLEAN and DIRTY */\n        assert( pageNotOnDirtyList(pCache, pPg) );/* CLEAN pages not on dirtylist */\n    }\n}\n";
    let expected = "void f()\n{\n    if(a) {\n        assert((pPg->flags & PGHDR_DIRTY) == 0);/* Cannot be both CLEAN and DIRTY */\n        assert(pageNotOnDirtyList(pCache, pPg));  /* CLEAN pages not on dirtylist */\n    }\n}\n";
    check(input, &["--pad-oper", "--unpad-paren"], expected);
}

#[test]
fn padded_initializer_row_shrinks_its_comment_gap() {
    let input = "static const int t[3][3] = {\n    {1024*1024*256, 1024*1024*64, 60}, /* slave */\n    {1024*1024*32, 1024*1024*8, 60} /* pubsub */\n};\n";
    let expected = "static const int t[3][3] = {\n    {1024 * 1024 * 256, 1024 * 1024 * 64, 60}, /* slave */\n    {1024 * 1024 * 32, 1024 * 1024 * 8, 60} /* pubsub */\n};\n";
    check(input, &["--pad-oper"], expected);
    check(expected, &["--pad-oper"], expected);
}

#[test]
fn added_brace_after_unpadded_header_takes_one_space() {
    let input = "void f()\n{\n    if( a & B  ) z = 1;\n    if( a  ) z = 1;\n    if( a) z = 1;\n}\n";
    let expected = "void f()\n{\n    if (a & B) {\n        z = 1;\n    }\n    if (a) {\n        z = 1;\n    }\n    if (a) {\n        z = 1;\n    }\n}\n";
    check(
        input,
        &["--add-braces", "--unpad-paren", "--pad-header"],
        expected,
    );
    check(
        expected,
        &["--add-braces", "--unpad-paren", "--pad-header"],
        expected,
    );
}

#[test]
fn padded_header_moves_a_brace_comment_back_to_its_column() {
    let input = "void f()\n{\n    if( pTerm->eOperator & WO_EQUIV  ) zType[1] = 1;\n  if( iDepth>1 ){   /*OPTIMIZATION-IF-TRUE*/\n    x();\n  }\n  if( p==0 ){     /*OPTIMIZATION-IF-FALSE*/\n    x();\n  }\n}\n";
    let expected = "void f()\n{\n    if (pTerm->eOperator & WO_EQUIV) {\n        zType[1] = 1;\n    }\n    if (iDepth > 1) { /*OPTIMIZATION-IF-TRUE*/\n        x();\n    }\n    if (p == 0) {   /*OPTIMIZATION-IF-FALSE*/\n        x();\n    }\n}\n";
    check(
        input,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
    check(
        expected,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
}

#[test]
fn comment_after_a_guarded_macro_with_an_if_stays_at_column_one() {
    let input = "#if !defined(luai_nummod)\n#define luai_nummod(L,a,b,m)  \\\n  { (void)L; (m) = l_mathop(fmod)(a,b); \\\n    if (((m) > 0) ? (b) < 0 : ((m) < 0 && (b) > 0)) (m) += (b); }\n#endif\n\n/* exponentiation */\n#if !defined(luai_numpow)\n#define X 1\n#endif\n";
    let expected = "#if !defined(luai_nummod)\n#define luai_nummod(L,a,b,m)  \\\n    { (void)L; (m) = l_mathop(fmod)(a,b); \\\n        if (((m) > 0) ? (b) < 0 : ((m) < 0 && (b) > 0)) (m) += (b); }\n#endif\n\n/* exponentiation */\n#if !defined(luai_numpow)\n#define X 1\n#endif\n";
    check(
        input,
        &["--indent-col1-comments", "--indent-preproc-define"],
        expected,
    );
    check(
        expected,
        &["--indent-col1-comments", "--indent-preproc-define"],
        expected,
    );
}

#[test]
fn unpad_paren_removes_the_space_after_a_bracket() {
    let input = "void f()\n{\n    x = a[ (b) ];\n    x = a[ b ];\n    x = f( (b) );\n    x = - (b);\n    x = ! (b);\n    x = a , (b);\n    x = a [ (b) ];\n}\n";
    let expected = "void f()\n{\n    x = a[(b) ];\n    x = a[ b ];\n    x = f((b));\n    x = - (b);\n    x = !(b);\n    x = a, (b);\n    x = a [(b) ];\n}\n";
    check(input, &["--unpad-paren"], expected);
    check(expected, &["--unpad-paren"], expected);
}

#[test]
fn function_named_foreach_is_no_header() {
    let input = "static int foreach (lua_State *L) {\n  x();\n  return 0;\n}\n";
    let expected = "static int foreach (lua_State *L)\n{\n    x();\n    return 0;\n}\n";
    check(
        input,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
    check(
        expected,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
}

#[test]
fn attached_return_types_join_pointer_runs_and_split_past_the_width() {
    let input = "const jim_subcmd_type *\nJim_ParseSubCmd(Jim_Interp *interp, const jim_subcmd_type *command_table, int argc, Jim_Obj *const *argv);\nint\nshort_one(int a);\nchar **\nngx_set_environment(ngx_cycle_t *cycle, ngx_uint_t *last)\n{\n    return 0;\n}\n";
    let expected = "const jim_subcmd_type *Jim_ParseSubCmd(Jim_Interp *interp, const jim_subcmd_type *command_table, int argc,\n                                       Jim_Obj *const *argv);\nint short_one(int a);\nchar **ngx_set_environment(ngx_cycle_t *cycle, ngx_uint_t *last)\n{\n    return 0;\n}\n";
    check(
        input,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
    check(
        expected,
        &[
            "--style=1tbs",
            "--mode=c",
            "--lineend=linux",
            "--convert-tabs",
            "--indent=spaces=4",
            "--indent-switches",
            "--indent-preprocessor",
            "--indent-preproc-define",
            "--indent-col1-comments",
            "--add-braces",
            "--pad-oper",
            "--pad-comma",
            "--pad-header",
            "--unpad-paren",
            "--break-one-line-headers",
            "--break-after-logical",
            "--align-pointer=name",
            "--align-reference=name",
            "--attach-closing-while",
            "--attach-return-type",
            "--attach-return-type-decl",
            "--min-conditional-indent=0",
            "--max-continuation-indent=80",
            "--max-code-length=109",
        ],
        expected,
    );
}

#[test]
fn max_code_length_takes_the_last_comparison_or_ternary_split_that_fits() {
    let input = "void f()\n{\n    hdr_record_value(config.latency_histogram, (long)c->latency <= CONFIG_LATENCY_HISTOGRAM_MAX_VALUE ? (long)c->latency : CONFIG_LATENCY_HISTOGRAM_MAX_VALUE);\n    x = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <= bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ? cccccccccccccccccccccc : dddddddddddddddddddddd;\n}\n";
    let expected = "void f()\n{\n    hdr_record_value(config.latency_histogram,\n                     (long)c->latency <= CONFIG_LATENCY_HISTOGRAM_MAX_VALUE ? (long)c->latency :\n                     CONFIG_LATENCY_HISTOGRAM_MAX_VALUE);\n    x = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <= bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ? cccccccccccccccccccccc :\n        dddddddddddddddddddddd;\n}\n";
    check(input, &["--max-code-length=100"], expected);
    check(expected, &["--max-code-length=100"], expected);
}

#[test]
fn max_code_length_takes_a_late_paren_over_a_pointer_declarator() {
    let input = "static enum help_format parse_help_format(const char *format)\n{\n    return 0;\n}\nstatic ngx_uint_t ngx_stream_geo_delete_range(ngx_conf_t *cf, int start);\n";
    let expected = "static enum help_format parse_help_format(\n    const char *format)\n{\n    return 0;\n}\nstatic ngx_uint_t ngx_stream_geo_delete_range(\n    ngx_conf_t *cf, int start);\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_aligns_a_member_call_argument_to_its_paren() {
    let input = "void f()\n{\n    if (a) {\n        n = c->recv(c, s->buffer->last, s->buffer->end - s->buffer->last);\n    }\n}\nstatic size_t reqresAppendEncodedBuffer(client *c, char *buf, size_t len)\n{\n}\n";
    let expected = "void f()\n{\n    if (a) {\n        n = c->recv(c, s->buffer->last,\n                    s->buffer->end - s->buffer->last);\n    }\n}\nstatic size_t reqresAppendEncodedBuffer(client *c,\n                                        char *buf, size_t len)\n{\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_continues_arguments_after_a_split_open_paren() {
    let input = "void f(void)\n{\n\tusage_msg_optf(_(\"options %s and %s cannot be used together\"),\n\t\t       bbb);\n\tx = usage_msg_optf(_(\"options %s and %s cannot be used together\"),\n\t\t       bbb);\n}\n";
    let expected = "void f(void)\n{\n    usage_msg_optf(\n        _(\"options %s and %s cannot be used together\"),\n        bbb);\n    x = usage_msg_optf(\n            _(\"options %s and %s cannot be used together\"),\n            bbb);\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_replays_astyle_split_points() {
    let input = "void f(void)\n{\n    if (x) {\n        *counter = 0; /* clear counter to make the read callback restart */\n        if (nack == cg->pel_nack_tail) new_cg->pel_nack_tail = new_nack;\n        const char *azFmt[] = { \"%s\", \"%s-journal\", \"%s-wal\", \"%s-shm\" };\n    }\n}\n";
    let expected = "void f(void)\n{\n    if (x) {\n        *counter =\n            0; /* clear counter to make the read callback restart */\n        if (nack == cg->pel_nack_tail) new_cg->pel_nack_tail =\n                new_nack;\n        const char *azFmt[] = { \"%s\", \"%s-journal\", \"%s-wal\", \"%s-shm\" };\n    }\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_continues_struct_returning_heads_and_compound_assignments() {
    let input = "static struct commit *deref_without_lazy_fetch(const struct object_id *oid,\n\t\t\t\t\t       int mark_tags_complete_and_check_obj_db)\n{\n    nack->delivery_count += nack->delivery_count == LLONG_MAX ? 0 : 1;\n}\n";
    let expected = "static struct commit *deref_without_lazy_fetch(\n    const struct object_id *oid,\n    int mark_tags_complete_and_check_obj_db)\n{\n    nack->delivery_count += nack->delivery_count == LLONG_MAX ?\n                            0 : 1;\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_continues_declarators_at_the_second_word() {
    let input = "void f(void)\n{\n    ngx_uint_t             width, sign, hex, max_width, frac_width, scale, n;\n}\n";
    let expected = "void f(void)\n{\n    ngx_uint_t             width, sign, hex, max_width, frac_width, scale,\n                           n;\n}\n";
    check(input, &["--style=gnu", "--max-code-length=70"], expected);
    check(expected, &["--style=gnu", "--max-code-length=70"], expected);
}

#[test]
fn max_code_length_keeps_a_directive_block_after_a_brace_whole() {
    let input = "void f(void)\n{\n    for ( ;; ) {\n        if (rc == NGX_OK) {\n\n#if (NGX_DEBUG)\n            {\n            ngx_str_t  key, value;\n\n            key.len = ctx->header_name_end - ctx->header_name_start;\n            }\n#endif\n        }\n    }\n}\nvoid g(void)\n{\n    x = 1;\n    {\n        key.len = ctx->header_name_end - ctx->header_name_start;\n    }\n}\n";
    let expected = "void f(void)\n{\n    for ( ;; ) {\n        if (rc == NGX_OK) {\n\n#if (NGX_DEBUG)\n            {\n                ngx_str_t  key, value;\n\n                key.len = ctx->header_name_end - ctx->header_name_start;\n            }\n#endif\n        }\n    }\n}\nvoid g(void)\n{\n    x = 1;\n    {\n        key.len = ctx->header_name_end -\n                  ctx->header_name_start;\n    }\n}\n";
    check(input, &["--style=linux", "--max-code-length=50"], expected);
    check(
        expected,
        &["--style=linux", "--max-code-length=50"],
        expected,
    );
}

#[test]
fn max_code_length_splits_a_braced_header_whose_comment_overflows() {
    let input = "void f(void)\n{\n    if (ls->lookahead.token != TK_EOS) {  /* is there a look-ahead token? */\n        x = 1;\n    } else if (opts->force_detach || !new_branch_info->path) {\t/* No longer on any branch. */\n        y = 2;\n    }\n}\n";
    let expected = "void f(void)\n{\n    if (ls->lookahead.token !=\n            TK_EOS) {  /* is there a look-ahead token? */\n        x = 1;\n    } else if (opts->force_detach\n               || !new_branch_info->path) {\t/* No longer on any branch. */\n        y = 2;\n    }\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_splits_before_an_empty_body_and_measures_tails_unindented() {
    let input = "static int pushline (lua_State *L, int firstline) {\n  if (firstline && b[0] == '=')  /* first line starts with `=' ? */\n    lua_pushfstring(L, \"return %s\", b+1);\n}\nvoid f(void) {\n      for(i=3, c=z[2]; (c!='*' || z[i]!='/') && (c=z[i])!=0; i++){}\n}\n";
    let expected = "static int pushline (lua_State *L, int firstline)\n{\n    if (firstline\n            && b[0] == '=')  /* first line starts with `=' ? */\n        lua_pushfstring(L, \"return %s\", b+1);\n}\nvoid f(void)\n{\n    for(i=3, c=z[2]; (c!='*' || z[i]!='/')\n            && (c=z[i])!=0; i++) {}\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_aligns_a_comment_before_a_split_statement_in_a_case_block() {
    let input = "void g(void)\n{\n  switch(x) {\n  case RTP_PARSE_CHANNEL: {\n      DEBUGASSERT(skip_len == 0);\n      /* we do not consume this byte, it is BODY data */\n      DEBUGF(infof(data, \"RTSP: invalid RTP channel %d, skipping\", idx));\n    break;\n  }\n  }\n}\n";
    let expected = "void g(void)\n{\n    switch(x) {\n    case RTP_PARSE_CHANNEL: {\n        DEBUGASSERT(skip_len == 0);\n        /* we do not consume this byte, it is BODY data */\n        DEBUGF(infof(data, \"RTSP: invalid RTP channel %d, skipping\",\n                     idx));\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_aligns_inside_parens_after_a_shift() {
    let input = "static int64_t value_from_index(int32_t bucket_index, int32_t sub_bucket_index, int32_t unit_magnitude)\n{\n    return ((int64_t) sub_bucket_index) << (bucket_index + unit_magnitude);\n}\n";
    let expected = "static int64_t value_from_index(int32_t bucket_index,\n                                int32_t sub_bucket_index, int32_t unit_magnitude)\n{\n    return ((int64_t) sub_bucket_index) << (bucket_index +\n                                            unit_magnitude);\n}\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn max_code_length_continues_a_run_in_body_assignment_past_the_header() {
    let input = "void f(void)\n{\n    if (ar && arIsPtr(v)) ar->alloc_size += zmalloc_size(v);\n    if (ar && arIsPtr(v)) ar->alloc_size = zmalloc_size(v);\n}\n";
    let expected = "void f(void)\n{\n    if (ar && arIsPtr(v)) ar->alloc_size +=\n            zmalloc_size(v);\n    if (ar && arIsPtr(v)) ar->alloc_size =\n            zmalloc_size(v);\n}\n";
    check(input, &["--style=linux", "--max-code-length=50"], expected);
    check(
        expected,
        &["--style=linux", "--max-code-length=50"],
        expected,
    );
}

#[test]
fn max_code_length_continues_a_member_declarator_at_the_second_word() {
    let input = "struct WhereLevel {\n  u8 iFrom;             /* Which entry in the FROM clause */\n  u8 op, p3, p5;        /* Opcode, P3 & P5 of the opcode that ends the loop */\n};\n";
    let expected = "struct WhereLevel {\n    u8 iFrom;             /* Which entry in the FROM clause */\n    u8 op, p3,\n    p5;        /* Opcode, P3 & P5 of the opcode that ends the loop */\n};\n";
    check(input, &["--style=kr", "--max-code-length=60"], expected);
    check(expected, &["--style=kr", "--max-code-length=60"], expected);
}

#[test]
fn member_declarators_after_a_short_type_continue_at_the_member() {
    let input = "struct A {\n    u8 op, p3,\n       p5;\n    unsigned long x, y,\n    z;\n};\n";
    let expected =
        "struct A {\n    u8 op, p3,\n    p5;\n    unsigned long x, y,\n             z;\n};\n";
    check(input, &["--style=kr"], expected);
}

#[test]
fn padding_parens_outside_keeps_the_spacing_after_an_ampersand_past_a_paren() {
    let input = "void f(void)\n{\n    x = (a)&b;\n    x = (a) &b;\n    x = (a)& b;\n    x = (a) & b;\n    g((voidp)&x, (voidp) &x);\n}\n";
    let expected = "void f (void)\n{\n    x = (a) &b;\n    x = (a) &b;\n    x = (a) & b;\n    x = (a) & b;\n    g ( (voidp) &x, (voidp) &x);\n}\n";
    check(input, &["--pad-paren-out"], expected);
    check(input, &["--pad-oper", "--pad-paren-out"], expected);
}

#[test]
fn tab_indent_indents_an_initializer_brace_of_brace_indenting_styles_with_tabs() {
    let input = "static sqlite3_module templatevtabModule =\n\t{\n\t/* iVersion    */ 0,\n\t0\n\t};\nvoid f(void)\n\t{\n\tstruct part parts[] =\n\t\t{\n\t\t\t{ 1, 2 }\n\t\t};\n\t}\n";
    check(input, &["--style=whitesmith", "--indent=tab=8"], input);
}

#[test]
fn tab_indent_indents_a_statement_broken_off_its_case_label_with_tabs() {
    let input = "int f(int idx)\n{\n  switch (idx) {\n    case LUA_REGISTRYINDEX: return registry(L);\n    default: return 4;\n  }\n}\n";
    let expected = "int f(int idx)\n\t{\n\tswitch (idx)\n\t\t{\n\t\tcase LUA_REGISTRYINDEX:\n\t\t\treturn registry(L);\n\t\tdefault:\n\t\t\treturn 4;\n\t\t}\n\t}\n";
    check(input, &["--style=whitesmith", "--indent=tab=4"], expected);
}

#[test]
fn a_dereferenced_increment_after_a_logical_operator_keeps_its_star() {
    let input = "void f(void)\n{\n    if (place[1] != 0 && *++place == 1 && place[1] == 0)\n        x = 1;\n    z = a && *--p;\n}\n";
    let expected = "void f(void) {\n    if (place[1] != 0 && *++place == 1 && place[1] == 0)\n        x = 1;\n    z = a && *--p;\n}\n";
    check(input, &["--style=google", "--align-pointer=type"], expected);
    check(
        input,
        &["--style=google", "--align-pointer=middle"],
        expected,
    );
}

#[test]
fn a_paren_and_apostrophe_in_a_comment_row_continue_nothing() {
    let input = "#endif\n/* since \"static\" is used, we\n   define it (compile with -Dlocal if your debugger can't find it) */\n\n/* gz* functions */\nextern voidp  malloc(uInt size);\n";
    check(input, &["--style=stroustrup"], input);
}

#[test]
fn deleting_empty_lines_keeps_breaking_a_header_off_a_one_line_comment() {
    let input = "int f(void)\n{\n    char s[81];\n    /* get limits */\n\n    if (a) {\n        x = 1;\n    }\n    y = 2;\n    // c2\n\n    z = 3;\n}\n";
    let expected = "int f(void)\n{\n    char s[81];\n    /* get limits */\n\n    if (a) {\n        x = 1;\n    }\n\n    y = 2;\n    // c2\n    z = 3;\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks", "--delete-empty-lines"],
        expected,
    );
}

#[test]
fn deleting_empty_lines_keeps_those_around_comments_before_a_broken_block() {
    let input = "int f(void)\n{\n    int i;\n\n    /*\n     * long\n     */\n\n    if (n < 3)\n        return 0;\n    acceptfail = 0;\n\n    /* if a command\n       starts */\n\n    if (cmd[0] == 1) {\n        x = 1;\n    }\n}\n";
    let expected = "int f(void)\n{\n    int i;\n\n    /*\n     * long\n     */\n\n    if (n < 3)\n        return 0;\n\n    acceptfail = 0;\n\n    /* if a command\n       starts */\n\n    if (cmd[0] == 1) {\n        x = 1;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks", "--delete-empty-lines"],
        expected,
    );
}

#[test]
fn deleting_empty_lines_keeps_those_of_a_directive_block_after_a_brace() {
    let input = "void f(void)\n{\n    for ( ;; ) {\n        if (rc == NGX_OK) {\n\n#if (NGX_DEBUG)\n            {\n                ngx_str_t  key, value;\n\n                key.len = 1;\n            }\n#endif\n        }\n    }\n}\n";
    let expected = "void f(void)\n{\n    for ( ;; ) {\n        if (rc == NGX_OK) {\n#if (NGX_DEBUG)\n            {\n                ngx_str_t  key, value;\n\n                key.len = 1;\n            }\n#endif\n        }\n    }\n}\n";
    check(input, &["--style=kr", "--delete-empty-lines"], expected);
}

#[test]
fn breaking_closing_header_blocks_skips_comment_only_blocks_and_else_chains_over_directives() {
    let input = "void f(void)\n{\n    if (a) {\n        /* nothing */\n    } else if (b) {\n        x = 1;\n    } else\n#ifdef T\n        if (c) {\n            y = 1;\n        }\n#endif\n    z = 2;\n}\n";
    let expected = "void f(void)\n{\n    if (a)\n    {\n        /* nothing */\n    }\n    else if (b)\n    {\n        x = 1;\n    }\n\n    else\n#ifdef T\n        if (c)\n        {\n            y = 1;\n        }\n\n#endif\n    z = 2;\n}\n";
    check(input, &["--style=allman", "--break-blocks=all"], expected);
}

#[test]
fn breaking_closing_header_blocks_keeps_else_after_a_trailing_block_comment() {
    let input = "int f(void)\n{\n    if (a)  /* c1 */\n        return 1;  /* c2 */\n    else\n    {\n        x = 1;\n    }\n    if (a)\n        return 1;\n    else\n    {\n        x = 1;\n    }\n    if (a)\n        return 1;  /* c2 */\n    else\n        x = 2;\n}\nint g(void)\n{\n    if (a)\n        return 1;  // c2\n    else\n        x = 2;\n    if (a)\n        return 1;  /* c2 */\n    else\n        x = 2;\n}\n";
    let expected = "int f(void)\n{\n    if (a)  /* c1 */\n        return 1;  /* c2 */\n    else\n    {\n        x = 1;\n    }\n\n    if (a)\n        return 1;\n\n    else\n    {\n        x = 1;\n    }\n\n    if (a)\n        return 1;  /* c2 */\n    else\n        x = 2;\n}\nint g(void)\n{\n    if (a)\n        return 1;  // c2\n\n    else\n        x = 2;\n\n    if (a)\n        return 1;  /* c2 */\n    else\n        x = 2;\n}\n";
    check(input, &["--style=allman", "--break-blocks=all"], expected);
}

#[test]
fn breaking_blocks_separates_a_block_closed_before_a_trailing_comment() {
    let input = "int f(void)\n{\n    while (b) {\n        x = 1;\n    } /* c4 */\n    y = 4;\n    while (b) {\n        x = 1;\n    } // c5\n    y = 5;\n}\n";
    let expected = "int f(void)\n{\n    while (b) {\n        x = 1;\n    } /* c4 */\n\n    y = 4;\n\n    while (b) {\n        x = 1;\n    } // c5\n\n    y = 5;\n}\n";
    check(input, &["--style=kr", "--break-blocks"], expected);
}

#[test]
fn breaking_blocks_reads_comment_led_code_and_directives_after_case_labels() {
    let input = "void f(void)\n{\n    edata_t *edata = extent_recycle(tsdn, pac,\n                                    /* growing_retained */ true, guarded);\n    if (edata != NULL) {\n        x = 1;\n    }\n    switch (ch) {\n    case 1:\n#if (NGX_WIN32)\n        if (a) {\n            b = 1;\n        }\n#endif\n        break;\n    }\n}\n";
    let expected = "void f(void)\n{\n    edata_t *edata = extent_recycle(tsdn, pac,\n                                    /* growing_retained */ true, guarded);\n\n    if (edata != NULL) {\n        x = 1;\n    }\n\n    switch (ch) {\n    case 1:\n#if (NGX_WIN32)\n        if (a) {\n            b = 1;\n        }\n\n#endif\n        break;\n    }\n}\n";
    check(input, &["--style=kr", "--break-blocks"], expected);
}

#[test]
fn breaking_blocks_separates_a_while_loop_after_an_earlier_do_while() {
    let input = "int f(void)\n{\n    do {\n        x = 1;\n    } while (isdigit(c));\n    if (check_next(ls, \"Ee\"))  /* `E'? */\n        check_next(ls, \"+-\");  /* optional exponent sign */\n    while (isalnum(c))\n        save(ls);\n}\n";
    let expected = "int f(void)\n{\n    do {\n        x = 1;\n    } while (isdigit(c));\n\n    if (check_next(ls, \"Ee\"))  /* `E'? */\n        check_next(ls, \"+-\");  /* optional exponent sign */\n\n    while (isalnum(c))\n        save(ls);\n}\n";
    check(input, &["--style=kr", "--break-blocks"], expected);
}

#[test]
fn breaking_blocks_separates_no_fall_through_labels_across_directives() {
    let input = "int f(int e)\n{\n    switch (e) {\n    case 1:\n    case 2:\n#ifdef X\n    case 3:\n#endif\n    case 4:\n        return 1;\n    default:\n        return 0;\n    }\n}\n";
    let expected = "int f(int e)\n{\n    switch (e) {\n    case 1:\n    case 2:\n#ifdef X\n    case 3:\n#endif\n    case 4:\n        return 1;\n\n    default:\n        return 0;\n    }\n}\n";
    check(input, &["--style=kr", "--break-blocks"], expected);
}

#[test]
fn deleting_empty_lines_keeps_one_before_an_attached_closing_header_after_a_comment() {
    let input = "int f(void)\n{\n    if (a) {\n        x = 1;\n        /* Note: the key is also present */\n\n    } else {\n        y = 2;\n    }\n}\n";
    let expected = "int f(void) {\n    if (a) {\n        x = 1;\n        /* Note: the key is also present */\n\n    } else {\n        y = 2;\n    }\n}\n";
    check(
        input,
        &[
            "--style=google",
            "--delete-empty-lines",
            "--break-blocks=all",
        ],
        expected,
    );
}

#[test]
fn breaking_blocks_separates_a_statement_macro_block_after_a_braceless_body() {
    let input = "void f(void)\n{\n    if (s_new == NULL)\n        cs_new = g(s);\n    TAILQ_FOREACH(c, &clients, entry)\n    {\n        x = 1;\n    }\n}\nvoid g(void)\n{\n    if (s_new == NULL &&\n            (d == 1 || d == 2))\n        cs_new = g(s);\n\n    TAILQ_FOREACH(c, &clients, entry)\n    {\n        x = 1;\n    }\n}\n";
    let expected = "void f(void)\n{\n    if (s_new == NULL)\n        cs_new = g(s);\n\n    TAILQ_FOREACH(c, &clients, entry)\n    {\n        x = 1;\n    }\n}\nvoid g(void)\n{\n    if (s_new == NULL &&\n            (d == 1 || d == 2))\n        cs_new = g(s);\n\n    TAILQ_FOREACH(c, &clients, entry)\n    {\n        x = 1;\n    }\n}\n";
    check(input, &["--style=allman", "--break-blocks"], expected);
}

#[test]
fn a_comment_row_starting_with_case_opens_no_case_label() {
    let input = "#ifndef X\n    /*\n    case iteration has failed, or a value.\n*/\n#endif\n";
    let expected = "#ifndef X
/*
case iteration has failed, or a value.
*/
#endif
";
    check(input, &["--style=kr", "--indent-preproc-cond"], expected);
}

#[test]
fn the_word_new_in_a_string_continues_no_new_expression() {
    let input = "static struct option opts[] = {\n        OPT_BOOL(0, \"a\", &b,\n            N_(\"don't checkout new files\")),\n        OPT_BOOL(0, \"c\", &d,\n            N_(\"update stat\")),\n};\nvoid f(void)\n{\n        g(a,\n            N_(\"don't checkout\")),\n        h(b,\n            c);\n}\n";
    let expected = "static struct option opts[] = {\n    OPT_BOOL(0, \"a\", &b,\n        N_(\"don't checkout new files\")),\n    OPT_BOOL(0, \"c\", &d,\n        N_(\"update stat\")),\n};\nvoid f(void)\n{\n    g(a,\n        N_(\"don't checkout\")),\n        h(b,\n            c);\n}\n";
    check(input, &["--style=linux", "--indent-after-parens"], expected);
}

#[test]
fn a_case_label_after_the_closing_brace_of_the_case_before_keeps_its_column() {
    let input = "void f(int i)\n{\n  switch (i)\n    {\n    case 2:\n      {\n        x = 1;\n        break;\n      } case 3:\n      {\n        y = 2;\n        break;\n      }\n    }\n  z = 3;\n}\n";
    check(
        input,
        &["--style=gnu", "--keep-one-line-statements"],
        "void f(int i)\n{\n    switch (i)\n        {\n        case 2:\n        {\n            x = 1;\n            break;\n        } case 3:\n        {\n            y = 2;\n            break;\n        }\n        }\n    z = 3;\n}\n",
    );
    check(
        input,
        &["--style=kr", "--keep-one-line-statements"],
        "void f(int i)\n{\n    switch (i) {\n    case 2: {\n        x = 1;\n        break;\n    } case 3: {\n        y = 2;\n        break;\n    }\n    }\n    z = 3;\n}\n",
    );
    check(
        input,
        &["--style=whitesmith", "--keep-one-line-statements"],
        "void f(int i)\n    {\n    switch (i)\n        {\n        case 2:\n            {\n            x = 1;\n            break;\n            } case 3:\n            {\n            y = 2;\n            break;\n            }\n        }\n    z = 3;\n    }\n",
    );
}

#[test]
fn an_added_one_line_block_under_a_braceless_header_leaves_the_next_block_level() {
    let input = "void f(void)\n{\n\tfor (; s2 < x; s2++)\n\t\tif (e() < 0)\n\t\t\treturn -1;\n\n\tfor (;; x = y) {\n\t\ta();\n\t}\n}\n";
    check(
        input,
        &["--style=allman", "--add-one-line-braces"],
        "void f(void)\n{\n    for (; s2 < x; s2++)\n        if (e() < 0)\n        { return -1; }\n\n    for (;; x = y)\n    {\n        a();\n    }\n}\n",
    );
}

#[test]
fn a_header_split_after_its_paren_in_an_else_chain_across_directives_indents_its_body_one_level() {
    let input = "void f(void)\n{\n  if(a) {\n  }\n  else\n#ifdef B\n  if(b) {\n  }\n  else\n#endif\n#ifdef C\n  if(c) {\n    if(\n      d ||\n      e) {\n      auth = 1;\n    }\n  }\n#endif\n  g();\n}\n";
    check(
        input,
        &["--style=1tbs"],
        "void f(void)\n{\n    if(a) {\n    } else\n#ifdef B\n        if(b) {\n        } else\n#endif\n#ifdef C\n            if(c) {\n                if(\n                    d ||\n                    e) {\n                    auth = 1;\n                }\n            }\n#endif\n    g();\n}\n",
    );
}

#[test]
fn adding_braces_leaves_a_body_after_a_directive_braceless_and_indented() {
    let input = "void f(void)\n{\n#ifndef X\n  if(a) {\n    b();\n  }\n  else\n#else\n  (void)c;\n#endif\n\n  d = 1;\n\n  e();\n}\n";
    check(
        input,
        &["--style=1tbs"],
        "void f(void)\n{\n#ifndef X\n    if(a) {\n        b();\n    } else\n#else\n    (void)c;\n#endif\n\n        d = 1;\n\n    e();\n}\n",
    );
}

#[test]
fn a_logical_and_before_a_trailing_comment_gives_the_padding_back_under_pointer_alignment() {
    let input = "void f(void)\n{\n    while(!a &&      /* c1 */\n          d) {\n        g();\n    }\n    while(a &&      /* c1 */\n          d) {\n        g();\n    }\n    while(a ||      /* c1 */\n          d) {\n        g();\n    }\n    while(a)      /* c1 */\n        g();\n}\n";
    check(
        input,
        &["--style=kr", "--pad-header", "--align-pointer=name"],
        "void f(void)\n{\n    while (!a &&     /* c1 */\n            d) {\n        g();\n    }\n    while (a &&     /* c1 */\n            d) {\n        g();\n    }\n    while (a ||     /* c1 */\n            d) {\n        g();\n    }\n    while (a)     /* c1 */\n        g();\n}\n",
    );
}

#[test]
fn a_comment_after_a_header_that_lost_its_closing_brace_keeps_its_column() {
    let input = "void f(void)\n{\n    if (a) {\n        b();\n    } else { /* c1 */\n        d();\n    }\n    if (a) {\n        b();\n    } else {   // c2\n        d();\n    }\n}\n";
    check(
        input,
        &["--style=stroustrup"],
        "void f(void)\n{\n    if (a) {\n        b();\n    }\n    else {   /* c1 */\n        d();\n    }\n    if (a) {\n        b();\n    }\n    else {     // c2\n        d();\n    }\n}\n",
    );
}

#[test]
fn a_continuation_row_led_by_plus_and_a_paren_with_a_trailing_comment_ends_its_statement() {
    let input = "int f(void)\n{\n  nByte = a\n         + (i+1);               /* c3 */\n  p = g(nByte);\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "int f(void) {\n    nByte = a\n            + (i+1);               /* c3 */\n    p = g(nByte);\n    }\n",
    );
}

#[test]
fn pico_breaks_the_brace_of_an_extern_c_block() {
    let input = "#ifdef __cplusplus\nextern \"C\" {\n#endif\n\nint f(void);\n\n#ifdef __cplusplus\n}\n#endif\n";
    check(
        input,
        &["--style=pico"],
        "#ifdef __cplusplus\nextern \"C\"\n{\n#endif\n\nint f(void);\n\n#ifdef __cplusplus\n}\n#endif\n",
    );
}

#[test]
fn pico_pads_no_closing_brace_after_a_comment_or_on_the_line_after_a_directive() {
    let input = "int f(void)\n{\n  int b;\n#if X\n  int c;\n#else\n  static int a[] = {1, 2};\n#endif\n  return 0;\n}\n";
    check(
        input,
        &["--style=pico"],
        "int f(void)\n{   int b;\n#if X\n    int c;\n#else\n    static int a[] = {1, 2};\n#endif\n    return 0; }\n",
    );
}

#[test]
fn lisp_pads_no_closing_brace_right_after_a_comment() {
    let input = "int uv__random_sysctl(void* buf, size_t buflen)\n{\n  static int name[] = {1 /*CTL_KERN*/, 40 /*KERN_RANDOM*/, 6 /*RANDOM_UUID*/};\n  static const char *azSub[] = {\"count\", \"depth\", 0};\n  int a[] = {1, 2};\n  return 0;\n}\n";
    check(
        input,
        &["--style=lisp"],
        "int uv__random_sysctl(void* buf, size_t buflen) {\n    static int name[] = {1 /*CTL_KERN*/, 40 /*KERN_RANDOM*/, 6 /*RANDOM_UUID*/};\n    static const char *azSub[] = {\"count\", \"depth\", 0 };\n    int a[] = {1, 2 };\n    return 0; }\n",
    );
}

#[test]
fn pico_runs_an_initializer_row_brace_into_its_block_comment() {
    let input = "static const struct t tests[] = {\n  { \"a\", 1 },\n  { /* query */\n    \"b\", 2 },\n  {\n    /* other */\n    \"c\", 3 },\n  { NULL, 0 }\n};\n";
    check(
        input,
        &["--style=pico"],
        "static const struct t tests[] = {\n    { \"a\", 1 },\n    {   /* query */\n        \"b\", 2 },\n    {   /* other */\n        \"c\", 3 },\n    { NULL, 0 } };\n",
    );
}

#[test]
fn horstmann_runs_an_initializer_row_brace_into_its_block_comment() {
    let input = "static const struct t tests[] = {\n  { \"a\", 1 },\n  { /* query */\n    \"b\", 2 },\n  {\n    /* other */\n    \"c\", 3 },\n  { NULL, 0 }\n};\n";
    check(
        input,
        &["--style=horstmann"],
        "static const struct t tests[] = {\n    { \"a\", 1 },\n    {   /* query */\n        \"b\", 2\n    },\n    {   /* other */\n        \"c\", 3\n    },\n    { NULL, 0 }\n};\n",
    );
}

#[test]
fn a_comment_only_one_line_block_adds_no_empty_line_before_its_closing_header() {
    let input = "void f(void)\n{\n    for (;;) {\n        if (a) b();\n        else if (c) { /* x */ }\n        else break;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks=all"],
        "void f(void)\n{\n    for (;;) {\n        if (a) b();\n\n        else if (c) { /* x */ }\n        else break;\n    }\n}\n",
    );
}

#[test]
fn an_empty_one_line_block_adds_no_empty_line_before_its_closing_header() {
    let input = "void f(void)\n{\n    for (;;) {\n        if (a) b();\n        else if (c) { }\n        else break;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks=all"],
        "void f(void)\n{\n    for (;;) {\n        if (a) b();\n\n        else if (c) { }\n        else break;\n    }\n}\n",
    );
}

#[test]
fn pico_breaks_the_block_after_a_one_line_header_run_into_the_brace() {
    let input = "void f(client *c)\n{\n    if (c->x == 0) return;\n    uint64_t timeout = c->y;\n    unsigned char buf[4];\n    g(buf);\n}\n";
    check(
        input,
        &["--style=pico", "--break-blocks"],
        "void f(client *c)\n{   if (c->x == 0) return;\n\n    uint64_t timeout = c->y;\n    unsigned char buf[4];\n    g(buf); }\n",
    );
}

#[test]
fn break_blocks_separates_case_labels_kept_on_one_line_with_their_statements() {
    let input = "int f(int c)\n{\n    int ret = 0;\n    switch (c) {\n        case 1: ret = 1; break;\n        case 2: ret = 2; break;\n        case 3: ret = 3; break;\n    }\n    return ret;\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks", "--keep-one-line-statements"],
        "int f(int c)\n{\n    int ret = 0;\n\n    switch (c) {\n    case 1: ret = 1; break;\n\n    case 2: ret = 2; break;\n\n    case 3: ret = 3; break;\n    }\n\n    return ret;\n}\n",
    );
}

#[test]
fn an_added_one_line_block_after_a_comment_stands_at_its_header() {
    let input = "void f(void)\n{\n\tif (a)\n\t\t/* c1 */\n\t\tx = 1;\n\telse\n\t\t/* c2 */\n\t\tx = 2;\n\tif (b)\n\t\ty();\n}\n";
    check(
        input,
        &["--style=kr", "--add-one-line-braces"],
        "void f(void)\n{\n    if (a)\n        /* c1 */\n    { x = 1; }\n    else\n        /* c2 */\n    { x = 2; }\n    if (b)\n    { y(); }\n}\n",
    );
}

#[test]
fn an_added_one_line_block_in_a_nested_case_body_takes_the_case_indent() {
    let input = "void f(void)\n{\n\tswitch (p_ch) {\n\tcase 1:\n\t\tif (a)\n\t\t\treturn;\n\t\tcontinue;\n\tcase 2:\n\t\tif (b) {\n\t\t\tif (c)\n\t\t\t\tx = 1;\n\t\t}\n\t}\n}\n";
    check(
        input,
        &["--style=kr", "--indent-switches", "--add-one-line-braces"],
        "void f(void)\n{\n    switch (p_ch) {\n        case 1:\n            if (a)\n            { return; }\n            continue;\n        case 2:\n            if (b) {\n                if (c)\n                { x = 1; }\n            }\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_a_statement_given_one_line_braces_stays_after_the_block() {
    let input = "void f(void)\n{\n    if (a) b(); /* c1 */\n    if (a)\n        b(); /* c2 */\n    if (a)\n        b(); // c3\n    x();\n}\n";
    check(
        input,
        &["--style=kr", "--add-one-line-braces"],
        "void f(void)\n{\n    if (a) { b(); } /* c1 */\n    if (a)\n    { b(); } /* c2 */\n    if (a)\n    { b(); } // c3\n    x();\n}\n",
    );
}

#[test]
fn an_added_one_line_block_in_a_braced_case_stands_at_its_header() {
    let input = "void f(int t)\n{\n    switch (t) {\n    case 1: {\n        if (a)\n            b();\n        c();\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--add-one-line-braces"],
        "void f(int t)\n{\n    switch (t) {\n    case 1: {\n        if (a)\n        { b(); }\n        c();\n        break;\n    }\n    }\n}\n",
    );
}

#[test]
fn an_added_one_line_block_keeps_its_header_level_and_comment_gap() {
    let input = "int f(int c)\n{\n    if (a)\n        /* if non-blocking input stalled,\n           return */\n        return 0;\n    if (b)\n        return 1;  /* two */\n    return 2;\n}\n";
    check(
        input,
        &["--style=kr", "--add-one-line-braces"],
        "int f(int c)\n{\n    if (a)\n        /* if non-blocking input stalled,\n           return */\n    { return 0; }\n    if (b)\n    { return 1; }  /* two */\n    return 2;\n}\n",
    );
}

#[test]
fn an_added_one_line_block_nested_in_a_case_body_stands_at_its_header() {
    let input = "void f(void)\n{\n\tswitch (state) {\n\tcase 2:\n\t\tfor (; ch; ch++)\n\t\t\tif (!x)\n\t\t\t\tbreak;\n\t\ty();\n\t}\n\tfor (; ch; ch++)\n\t\tif (!x)\n\t\t\tbreak;\n}\n";
    check(
        input,
        &["--style=kr", "--add-one-line-braces"],
        "void f(void)\n{\n    switch (state) {\n    case 2:\n        for (; ch; ch++)\n            if (!x)\n            { break; }\n        y();\n    }\n    for (; ch; ch++)\n        if (!x)\n        { break; }\n}\n",
    );
}

#[test]
fn gnu_indents_an_added_one_line_block_of_an_else_in_a_case_block() {
    let input = "void f(int t)\n{\n\tswitch (t) {\n\tcase 0:\n\t{\n\t\tif (x) {\n\t\t\tif (a) {\n\t\t\t\tb();\n\t\t\t} else\n\t\t\t\tc();\n\t\t}\n\t\tbreak;\n\t}\n\t}\n}\n";
    check(
        input,
        &["--style=gnu", "--add-one-line-braces"],
        "void f(int t)\n{\n    switch (t)\n        {\n        case 0:\n        {\n            if (x)\n                {\n                    if (a)\n                        {\n                            b();\n                        }\n                    else\n                        { c(); }\n                }\n            break;\n        }\n        }\n}\n",
    );
}

#[test]
fn an_added_one_line_block_after_a_multiline_loop_header_body_stands_at_its_header() {
    let input = "void f(void) {\n  while (g(a,\n           NULL) != 0) {\n  ASSERT(s(d, \"file1\") == 0 ||\n         s(d, \"sub\") == 0);\n#ifdef X\n    if (!s(d, \"sub\"))\n      E(d, 1);\n    else\n      E(d, 2);\n#endif\n  }\n}\n";
    check(
        input,
        &["--style=whitesmith", "--add-one-line-braces"],
        "void f(void)\n    {\n    while (g(a,\n             NULL) != 0)\n        {\n        ASSERT(s(d, \"file1\") == 0 ||\n               s(d, \"sub\") == 0);\n#ifdef X\n        if (!s(d, \"sub\"))\n            { E(d, 1); }\n        else\n            { E(d, 2); }\n#endif\n        }\n    }\n",
    );
}

#[test]
fn an_added_one_line_block_of_a_nested_else_stands_at_the_else() {
    let input = "void f(void)\n{\n\tfor (i = 0; i < n; i++)\n\t\tif (i >= limit)\n\t\t\ta();\n\t\telse\n\t\t\tb();\n\tif (x) {\n\t\ty();\n\t}\n\telse\n\t\tz();\n}\n";
    check(
        input,
        &["--style=gnu", "--add-one-line-braces"],
        "void f(void)\n{\n    for (i = 0; i < n; i++)\n        if (i >= limit)\n            { a(); }\n        else\n            { b(); }\n    if (x)\n        {\n            y();\n        }\n    else\n        { z(); }\n}\n",
    );
}

#[test]
fn header_words_in_a_string_nest_no_header_on_the_line() {
    let input = "int T(void) {\n    if (!X(ctx,\"a do b\",62)) goto fail;\n    for (;;) if (a) {\n        b();\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--add-braces"],
        "int T(void)\n{\n    if (!X(ctx,\"a do b\",62)) {\n        goto fail;\n    }\n    for (;;) if (a) {\n            b();\n        }\n}\n",
    );
}

#[test]
fn an_added_closing_brace_in_a_case_block_ignores_an_earlier_else_line() {
    let input = "int f(void){\n  switch( c ){\n    case 1: {\n      while( c ){\n        if( a ){\n        }else{\n        }\n      }\n      if( c==0 ) return 1;\n    }\n  }\n}\n";
    check(
        input,
        &["--style=kr", "--add-braces"],
        "int f(void)\n{\n    switch( c ) {\n    case 1: {\n        while( c ) {\n            if( a ) {\n            } else {\n            }\n        }\n        if( c==0 ) {\n            return 1;\n        }\n    }\n    }\n}\n",
    );
}

#[test]
fn break_blocks_looks_past_empty_lines_for_a_header_after_a_comment_in_a_switch() {
    let input = "void f(void)\n{\n    switch (s) {\n    case 1:\n        if (x) {\n            const char *tr = g();\n            /* c1\n               c1b */\n\n            if (tr) {\n                b();\n            }\n        }\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks"],
        "void f(void)\n{\n    switch (s) {\n    case 1:\n        if (x) {\n            const char *tr = g();\n\n            /* c1\n               c1b */\n\n            if (tr) {\n                b();\n            }\n        }\n\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_directive_in_the_body_of_an_else_split_across_directives_takes_its_level() {
    let input = "void f(void)\n{\n#ifdef A\n    if (a) {\n        b();\n    } else\n#endif\n    if (c) {\n#ifndef X\n        if (d) {\n            e();\n        } else\n#endif\n        {\n            g();\n        }\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--indent-preproc-cond"],
        "void f(void)\n{\n    #ifdef A\n    if (a) {\n        b();\n    } else\n    #endif\n        if (c) {\n            #ifndef X\n            if (d) {\n                e();\n            } else\n            #endif\n            {\n                g();\n            }\n        }\n}\n",
    );
}

#[test]
fn removing_comment_prefixes_keeps_lines_a_tab_indents_past_the_opener() {
    let input = "void f(void)\n{\n/* first line\n\tsecond line\n *\tstar tab\n *  star sp\n   third */\n    int x;\n    /* a\n\tb\n     */\n}\n";
    check(
        input,
        &["--style=kr", "--remove-comment-prefix"],
        "void f(void)\n{\n    /*  first line\n    \tsecond line\n     \tstar tab\n        star sp\n        third */\n    int x;\n    /*  a\n        b\n    */\n}\n",
    );
}

#[test]
fn padding_operators_pads_the_commas_of_a_standalone_macro_invocation() {
    let input = "void f(void)\n{\n    g(a,b);\n}\nOP(JUMP_F,BRANCH,   1, 0)\nOP(APPEND, VARIABLE,1, 0)\nint x = h(a,b);\n";
    check(
        input,
        &["--style=kr", "--pad-oper"],
        "void f(void)\n{\n    g(a, b);\n}\nOP(JUMP_F, BRANCH,   1, 0)\nOP(APPEND, VARIABLE, 1, 0)\nint x = h(a, b);\n",
    );
}

#[test]
fn a_type_aligned_star_parts_from_a_parenthesized_declarator() {
    let input = "struct s {\n    char *(*get)(void);\n    void *(*create)(const char *, size_t);\n};\nvoid *(*g)(void *);\nstatic const char *(matchbuf[1]);\nvoid f(void *(*fn)(void *));\nint x = (0xFE&*p->q);\n";
    check(
        input,
        &["--style=kr", "--align-pointer=type"],
        "struct s {\n    char* (*get)(void);\n    void* (*create)(const char*, size_t);\n};\nvoid* (*g)(void*);\nstatic const char* (matchbuf[1]);\nvoid f(void* (*fn)(void*));\nint x = (0xFE&*p->q);\n",
    );
}

#[test]
fn a_middle_aligned_star_parts_from_a_parenthesized_declarator() {
    let input = "struct s {\n    char *(*get)(void);\n    void *(*create)(const char *, size_t);\n};\nvoid *(*g)(void *);\nstatic const char *(matchbuf[1]);\nvoid f(void *(*fn)(void *));\nint x = (0xFE&*p->q);\n";
    check(
        input,
        &["--style=kr", "--align-pointer=middle"],
        "struct s {\n    char * (*get)(void);\n    void * (*create)(const char *, size_t);\n};\nvoid * (*g)(void *);\nstatic const char * (matchbuf[1]);\nvoid f(void * (*fn)(void *));\nint x = (0xFE&*p->q);\n",
    );
}

#[test]
fn a_star_right_after_an_ampersand_dereferences() {
    let input = "void f(void)\n{\n    x = (a&*p);\n    x = (a & *p);\n    x = (a+*p);\n    x = (a|*p);\n    x = a**p;\n    x = (a&&*p);\n}\n";
    check(
        input,
        &["--style=kr", "--align-pointer=type"],
        "void f(void)\n{\n    x = (a&*p);\n    x = (a & *p);\n    x = (a+*p);\n    x = (a|*p);\n    x = a** p;\n    x = (a&&*p);\n}\n",
    );
}

#[test]
fn breaking_the_return_type_keeps_the_name_gap_and_follows_a_macro_above_an_empty_line() {
    let input = "static TValue *index2adr (lua_State *L, int idx) {\n  return 0;\n}\nTEST_END\n\nint main(void) {\n  return 0;\n}\n";
    check(
        input,
        &["--style=kr", "--break-return-type"],
        "static TValue *\nindex2adr (lua_State *L, int idx)\n{\n    return 0;\n}\nTEST_END\n\nint\nmain(void)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn standalone_macro_invocations_take_unpad_paren() {
    let input = "BENCHMARK_DECLARE (loop_count)\nBENCHMARK_DECLARE( loop_alive )\n";
    check(
        input,
        &["--style=kr", "--unpad-paren"],
        "BENCHMARK_DECLARE(loop_count)\nBENCHMARK_DECLARE(loop_alive)\n",
    );
}

#[test]
fn standalone_macro_invocations_take_pad_paren() {
    let input = "BENCHMARK_DECLARE (loop_count)\nBENCHMARK_DECLARE( loop_alive )\n";
    check(
        input,
        &["--style=kr", "--pad-paren"],
        "BENCHMARK_DECLARE ( loop_count )\nBENCHMARK_DECLARE ( loop_alive )\n",
    );
}

#[test]
fn unpadding_parens_keeps_a_trailing_comment_column_and_leaves_inner_comments() {
    let input = "void f(void)\n{\n    if (!g(L, o)) {  /* c1 */\n        h();\n    }\n    for (i = 0; i < n; /* void */ ) {\n        h();\n    }\n    while (1 /* exit */) {\n        h();\n    }\n    if (a) b();  /* c2 */\n    x = ( a + b );  /* c3 */\n}\n";
    check(
        input,
        &["--style=kr", "--unpad-paren"],
        "void f(void)\n{\n    if(!g(L, o)) {   /* c1 */\n        h();\n    }\n    for(i = 0; i < n; /* void */) {\n        h();\n    }\n    while(1 /* exit */) {\n        h();\n    }\n    if(a) b();   /* c2 */\n    x = (a + b);    /* c3 */\n}\n",
    );
}

#[test]
fn padding_parens_outside_spaces_a_close_paren_from_a_binary_operator() {
    let input = "void f(void)\n{\n    x = (v>>7)&0x7f;\n    y = (int)&z;\n    w = (a)*b;\n    q = (a)|(b);\n    p = (char *)*pp;\n    r = (a)<<2;\n}\n";
    check(
        input,
        &["--style=kr", "--pad-paren-out"],
        "void f (void)\n{\n    x = (v>>7) &0x7f;\n    y = (int) &z;\n    w = (a) *b;\n    q = (a) | (b);\n    p = (char *) *pp;\n    r = (a) <<2;\n}\n",
    );
}

#[test]
fn padding_parens_outside_aligns_initializer_rows_past_the_padded_first_paren() {
    let input = "void f(void)\n{\n\tuniform_gen_arg_t arg = {(uint64_t)(uintptr_t)&lg_range_test,\n\t    lg_range_test};\n}\n";
    check(
        input,
        &["--style=kr", "--pad-paren-out"],
        "void f (void)\n{\n    uniform_gen_arg_t arg = { (uint64_t) (uintptr_t)&lg_range_test,\n                              lg_range_test\n                            };\n}\n",
    );
}

#[test]
fn a_statement_broken_off_its_header_keeps_the_gap_before_its_comment() {
    let input = "void f(void)\n{\n    if (a) b();  /* c1 */\n    if (c) return; // c2\n    while (d) e();      /* c3 */\n}\n";
    check(
        input,
        &["--style=kr", "--break-one-line-headers"],
        "void f(void)\n{\n    if (a)\n        b();  /* c1 */\n    if (c)\n        return; // c2\n    while (d)\n        e();      /* c3 */\n}\n",
    );
}

#[test]
fn break_one_line_headers_breaks_a_switch_after_else() {
    let input = "void f(int i)\n{\n    if (a) {\n        b();\n    } else switch (i) {  /* c */\n        case 1:\n            break;\n    }\n    if (a) {\n        b();\n    } else for (;;) {\n        c();\n    }\n    if (a) b(); else while (x) { y(); }\n}\n";
    check(
        input,
        &["--style=kr", "--break-one-line-headers"],
        "void f(int i)\n{\n    if (a) {\n        b();\n    } else\n        switch (i) {  /* c */\n        case 1:\n            break;\n        }\n    if (a) {\n        b();\n    } else\n        for (;;) {\n            c();\n        }\n    if (a)\n        b();\n    else\n        while (x) {\n            y();\n        }\n}\n",
    );
}

#[test]
fn break_one_line_headers_breaks_a_switch_after_a_header() {
    let input = "void f(int i)\n{\n    if (a) switch (i) {\n        case 1:\n            break;\n        }\n    for (;;) switch (i) {\n        default:\n            break;\n        }\n}\n";
    check(
        input,
        &["--style=kr", "--break-one-line-headers"],
        "void f(int i)\n{\n    if (a)\n        switch (i) {\n        case 1:\n            break;\n        }\n    for (;;)\n        switch (i) {\n        default:\n            break;\n        }\n}\n",
    );
}

#[test]
fn break_blocks_parts_a_case_block_brace_from_a_directive_after_it() {
    let input = "void f(void)\n{\n    switch (c) {\n    case 0:\n        break;\n#if X\n    case 1: {\n        if (a) {\n            b();\n        }\n        break;\n    }\n#endif\n    case 2:\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks"],
        "void f(void)\n{\n    switch (c) {\n    case 0:\n        break;\n#if X\n\n    case 1: {\n        if (a) {\n            b();\n        }\n\n        break;\n    }\n\n#endif\n\n    case 2:\n        break;\n    }\n}\n",
    );
}

#[test]
fn padding_the_first_paren_outside_aligns_initializer_rows_past_it() {
    let input = "void f(void)\n{\n    int a[] = {(1), 2,\n        3};\n    int b[] = {((1)), 2,\n        3};\n}\n";
    check(
        input,
        &["--style=kr", "--pad-first-paren-out"],
        "void f (void)\n{\n    int a[] = { (1), 2,\n                3\n              };\n    int b[] = { ((1)), 2,\n                3\n              };\n}\n",
    );
}

#[test]
fn a_macro_body_row_after_a_return_with_token_pasting_stays_as_written() {
    let input =
        "#define M(t)\t\\\nint\t\\\nf(int a) {\t\\\n\treturn g_##t(a,\t\\\n\t    b);\t\\\n}\n";
    check(
        input,
        &["--style=kr"],
        "#define M(t)\t\\\nint\t\\\nf(int a) {\t\\\n\treturn g_##t(a,\t\\\n\t    b);\t\\\n}\n",
    );
}

#[test]
fn a_pointer_declaration_assignment_continuation_follows_indent_continuation_zero() {
    let input = "void f(void)\n{\n    struct w* w =\n        c(r);\n    struct w *w =\n        c(r);\n    struct w w =\n        c(r);\n    st w* w =\n        c(r);\n}\n";
    check(
        input,
        &["--indent-continuation=0"],
        "void f(void)\n{\n    struct w* w =\n    c(r);\n    struct w *w =\n    c(r);\n    struct w w =\n    c(r);\n    st w* w =\n    c(r);\n}\n",
    );
}

#[test]
fn a_pointer_declaration_assignment_continuation_follows_indent_continuation_three() {
    let input = "void f(void)\n{\n    struct w* w =\n        c(r);\n    struct w *w =\n        c(r);\n    struct w w =\n        c(r);\n    st w* w =\n        c(r);\n}\n";
    check(
        input,
        &["--indent-continuation=3"],
        "void f(void)\n{\n    struct w* w =\n                c(r);\n    struct w *w =\n                c(r);\n    struct w w =\n                c(r);\n    st w* w =\n                c(r);\n}\n",
    );
}

#[test]
fn a_semicolon_inside_a_define_for_header_keeps_the_header_open() {
    let input = "void f() {\n  for (int i = 0;\n       j; j = 0)\n    x(a,\n      b);\n}\n#define G(a) \\\n  for (int i = 0; j;     \\\n       j = 0)          \\\n    x(a, \\\n      b);\n#define jv_array_foreach(a, i, x) \\\n  for (int jv_len__ = jv_array_length(jv_copy(a)), i=0, jv_j__ = 1;     \\\n       jv_j__; jv_j__ = 0)                                              \\\n    for (jv x;                                                          \\\n         i < jv_len__ ?                                                 \\\n           (x = jv_array_get(jv_copy(a), i), 1) : 0;                    \\\n         i++)\n";
    check(
        input,
        &["--indent-preproc-define"],
        "void f() {\n    for (int i = 0;\n            j; j = 0)\n        x(a,\n          b);\n}\n#define G(a) \\\n    for (int i = 0; j;     \\\n            j = 0)          \\\n        x(a, \\\n          b);\n#define jv_array_foreach(a, i, x) \\\n    for (int jv_len__ = jv_array_length(jv_copy(a)), i=0, jv_j__ = 1;     \\\n            jv_j__; jv_j__ = 0)                                              \\\n        for (jv x;                                                          \\\n                i < jv_len__ ?                                                 \\\n                (x = jv_array_get(jv_copy(a), i), 1) : 0;                    \\\n                i++)\n",
    );
}

#[test]
fn a_kept_statement_after_a_case_block_brace_stays_at_the_brace() {
    let input =
        "void f()\n{\n    switch (a) {\n    case 1: {\n        x();\n    } break;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--keep-one-line-statements"],
        "void f()\n{\n    switch (a) {\n    case 1: {\n        x();\n    } break;\n    }\n}\n",
    );
}

#[test]
fn a_block_after_a_kept_statement_on_a_case_label_belongs_to_the_statement_kr() {
    let input = "void f()\n{\n    switch (a) {\n    default: assert(x); {\n        int y;\n        g(y);\n    }\n    h();\n    break;\n    case 2: q(); {\n        if (a) {\n            b();\n        }\n        c();\n    }\n    d();\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--keep-one-line-statements"],
        "void f()\n{\n    switch (a) {\n    default: assert(x); {\n            int y;\n            g(y);\n        }\n        h();\n        break;\n    case 2: q(); {\n            if (a) {\n                b();\n            }\n            c();\n        }\n        d();\n    }\n}\n",
    );
}

#[test]
fn a_block_after_a_kept_statement_on_a_case_label_belongs_to_the_statement_java_indented_cases() {
    let input = "void f()\n{\n    switch (a) {\n    default: assert(x); {\n        int y;\n        g(y);\n    }\n    h();\n    break;\n    case 2: q(); {\n        if (a) {\n            b();\n        }\n        c();\n    }\n    d();\n    }\n}\n";
    check(
        input,
        &[
            "--style=java",
            "--keep-one-line-statements",
            "--indent-cases",
            "--indent-switches",
        ],
        "void f() {\n    switch (a) {\n        default: assert(x); {\n                int y;\n                g(y);\n            }\n            h();\n            break;\n        case 2: q(); {\n                if (a) {\n                    b();\n                }\n                c();\n            }\n            d();\n    }\n}\n",
    );
}

#[test]
fn a_block_after_a_kept_statement_on_a_case_label_belongs_to_the_statement_allman() {
    let input = "void f()\n{\n    switch (a) {\n    default: assert(x); {\n        int y;\n        g(y);\n    }\n    h();\n    break;\n    case 2: q(); {\n        if (a) {\n            b();\n        }\n        c();\n    }\n    d();\n    }\n}\n";
    check(
        input,
        &["--style=allman", "--keep-one-line-statements"],
        "void f()\n{\n    switch (a)\n    {\n    default: assert(x);\n        {\n            int y;\n            g(y);\n        }\n        h();\n        break;\n    case 2: q();\n        {\n            if (a)\n            {\n                b();\n            }\n            c();\n        }\n        d();\n    }\n}\n",
    );
}

#[test]
fn a_star_run_closing_a_type_argument_is_a_pointer_with_pad_oper() {
    let input = "void f()\n{\n    g(char **, a);\n    g(char ** , a);\n    x = g(a**, b);\n}\nvoid g()\n{\n    const char **name = cast(const char**, dlsym(lib, \"x\"));\n    g(const char**, a);\n    g(char**, a);\n    g(a**, b);\n}\n";
    check(
        input,
        &["--pad-oper"],
        "void f()\n{\n    g(char **, a);\n    g(char **, a);\n    x = g(a**, b);\n}\nvoid g()\n{\n    const char **name = cast(const char**, dlsym(lib, \"x\"));\n    g(const char**, a);\n    g(char**, a);\n    g(a**, b);\n}\n",
    );
}

#[test]
fn a_star_run_closing_a_type_argument_is_a_pointer_with_type_alignment() {
    let input = "void f()\n{\n    g(char **, a);\n    g(char ** , a);\n    x = g(a**, b);\n}\nvoid g()\n{\n    const char **name = cast(const char**, dlsym(lib, \"x\"));\n    g(const char**, a);\n    g(char**, a);\n    g(a**, b);\n}\n";
    check(
        input,
        &["--align-pointer=type"],
        "void f()\n{\n    g(char**, a);\n    g(char**, a);\n    x = g(a**, b);\n}\nvoid g()\n{\n    const char** name = cast(const char**, dlsym(lib, \"x\"));\n    g(const char**, a);\n    g(char**, a);\n    g(a**, b);\n}\n",
    );
}

#[test]
fn a_star_run_closing_a_type_argument_is_a_pointer_with_name_alignment() {
    let input = "void f()\n{\n    g(char **, a);\n    g(char ** , a);\n    x = g(a**, b);\n}\nvoid g()\n{\n    const char **name = cast(const char**, dlsym(lib, \"x\"));\n    g(const char**, a);\n    g(char**, a);\n    g(a**, b);\n}\n";
    check(
        input,
        &["--pad-oper", "--align-pointer=name"],
        "void f()\n{\n    g(char **, a);\n    g(char **, a);\n    x = g(a **, b);\n}\nvoid g()\n{\n    const char **name = cast(const char **, dlsym(lib, \"x\"));\n    g(const char **, a);\n    g(char **, a);\n    g(a **, b);\n}\n",
    );
}

#[test]
fn a_bare_return_value_continues_by_indent_continuation() {
    let input = "int f()\n{\n    return\n        (a & 0xff) >> 56 |\n        b;\n}\nint g()\n{\n    return\n        a ? b : c;\n}\n";
    check(
        input,
        &["--indent-continuation=3"],
        "int f()\n{\n    return\n                (a & 0xff) >> 56 |\n                b;\n}\nint g()\n{\n    return\n                a ? b : c;\n}\n",
    );
}

#[test]
fn a_continued_case_label_goes_one_level_past_its_body() {
    let input = "int f()\n{\n    switch (mask) {\n    case A|B|\n        C|D:\n        return 1;\n    }\n}\n";
    check(
        input,
        &["--indent-continuation=3"],
        "int f()\n{\n    switch (mask) {\n    case A|B|\n            C|D:\n        return 1;\n    }\n}\n",
    );
}

#[test]
fn a_continued_indented_case_label_goes_one_level_past_its_body() {
    let input = "int f()\n{\n    switch (mask) {\n    case A|B|\n        C|D:\n        return 1;\n    }\n}\n";
    check(
        input,
        &["--indent-continuation=0", "--indent-switches"],
        "int f()\n{\n    switch (mask) {\n        case A|B|\n                C|D:\n            return 1;\n    }\n}\n",
    );
}

#[test]
fn a_leading_assignment_at_file_scope_goes_one_level_in() {
    let input = "static const char b[64+1]\n= \"ABC\";\nstatic int x\n    = 5;\nvoid f()\n{\n    int y\n        = 3;\n}\n";
    check(
        input,
        &["--indent-continuation=3"],
        "static const char b[64+1]\n    = \"ABC\";\nstatic int x\n    = 5;\nvoid f()\n{\n    int y\n        = 3;\n}\n",
    );
}

#[test]
fn an_else_body_split_by_an_empty_line_keeps_its_level_past_a_directive() {
    let input = "void f()\n{\n    if (a) {\n        x();\n    } else\n\n    if (b) {\n#ifdef X\n        if (c) {\n            y();\n        }\n#else\n        z();\n#endif\n        w();\n    }\n    v();\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    if (a) {\n        x();\n    } else\n\n        if (b) {\n#ifdef X\n            if (c) {\n                y();\n            }\n#else\n            z();\n#endif\n            w();\n        }\n    v();\n}\n",
    );
}

#[test]
fn a_line_splice_after_a_closing_brace_stays_on_its_line() {
    let input =
        "void f()\n{\n    if (a) {\n        b();\n    } else {\n        c();\n    }      \\\n  }\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    if (a) {\n        b();\n    } else {\n        c();\n    }      \\\n}\n",
    );
}

#[test]
fn a_type_header_ended_by_a_comment_takes_its_attached_brace_kr() {
    let input = "struct a\n{\n    int x;\n};\nstruct d /* c */\n{\n    int x;\n} v;\nunion u // c\n{\n    int x;\n};\nenum e /* c */\n{\n    A\n};\nclass C /* c */\n{\n    int x;\n};\n";
    check(
        input,
        &["--style=kr"],
        "struct a {\n    int x;\n};\nstruct d { /* c */\n    int x;\n} v;\nunion u { // c\n    int x;\n};\nenum e /* c */\n{\n    A\n};\nclass C /* c */\n{\n    int x;\n};\n",
    );
}

#[test]
fn a_type_header_ended_by_a_comment_takes_its_attached_brace_java() {
    let input = "struct a\n{\n    int x;\n};\nstruct d /* c */\n{\n    int x;\n} v;\nunion u // c\n{\n    int x;\n};\nenum e /* c */\n{\n    A\n};\nclass C /* c */\n{\n    int x;\n};\n";
    check(
        input,
        &["--style=java"],
        "struct a {\n    int x;\n};\nstruct d { /* c */\n    int x;\n} v;\nunion u { // c\n    int x;\n};\nenum e /* c */\n{\n    A\n};\nclass C { /* c */\n    int x;\n};\n",
    );
}

#[test]
fn a_type_header_ended_by_a_comment_takes_its_attached_brace_mozilla() {
    let input = "struct a\n{\n    int x;\n};\nstruct d /* c */\n{\n    int x;\n} v;\nunion u // c\n{\n    int x;\n};\nenum e /* c */\n{\n    A\n};\nclass C /* c */\n{\n    int x;\n};\n";
    check(
        input,
        &["--style=mozilla"],
        "struct a\n{\n    int x;\n};\nstruct d /* c */\n{\n    int x;\n} v;\nunion u { // c\n    int x;\n};\nenum e /* c */\n{\n    A\n};\nclass C /* c */\n{\n    int x;\n};\n",
    );
}

#[test]
fn a_comment_led_row_closing_a_macro_call_ends_its_statement() {
    let input = "void f()\n{\n    if (a) {\n        M(x,\n          /* clip */ true)\n        M(y,\n          /* clip */ false)\n        M(z,\n          b, true)\n        M(w,\n          c)\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    if (a) {\n        M(x,\n          /* clip */ true)\n        M(y,\n          /* clip */ false)\n        M(z,\n          b, true)\n        M(w,\n          c)\n    }\n}\n",
    );
}

#[test]
fn a_statement_after_a_case_block_stands_at_the_indented_case_body() {
    let input = "void f()\n{\n    switch(t) {\n    case 1: {\n        int s;\n    }\n    break;\n    case 2:\n        x();\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--indent-cases"],
        "void f()\n{\n    switch(t) {\n    case 1: {\n            int s;\n        }\n        break;\n    case 2:\n        x();\n    }\n}\n",
    );
}

#[test]
fn a_case_block_after_an_empty_else_block_keeps_its_indented_body() {
    let input = "void f()\n{\n    switch (w) {\n    case 1: {\n        if (a) {\n        }\n        else {\n        }\n    }\n    case 2: {\n        if (c) {\n            d();\n        }\n    }\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--indent-cases"],
        "void f()\n{\n    switch (w) {\n    case 1: {\n            if (a) {\n            } else {\n            }\n        }\n    case 2: {\n            if (c) {\n                d();\n            }\n        }\n    }\n}\n",
    );
}

#[test]
fn a_case_block_closer_with_a_semicolon_stands_at_the_indented_case_body() {
    let input = "void f()\n{\n    switch (w) {\n    case 1: {\n        x();\n        break;\n    };\n    case 2:\n        y();\n    }\n}\n";
    check(
        input,
        &["--style=ratliff", "--indent-cases"],
        "void f() {\n    switch (w) {\n        case 1: {\n                x();\n                break;\n                };\n        case 2:\n            y();\n        }\n    }\n",
    );
}

#[test]
fn a_comment_before_a_commented_case_block_closer_keeps_its_column() {
    let input = "void f()\n{\n    switch (w) {\n    case 1: {\n        if (u) {\n            set();\n            break;\n        }\n        /* else... */\n    }  /* FALLTHROUGH */\n    case 2:\n        y();\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--indent-cases"],
        "void f()\n{\n    switch (w) {\n    case 1: {\n            if (u) {\n                set();\n                break;\n            }\n            /* else... */\n        }  /* FALLTHROUGH */\n    case 2:\n        y();\n    }\n}\n",
    );
}

#[test]
fn a_commented_case_block_closer_keeps_its_column_before_a_label() {
    let input = "void f()\n{\n    switch (w) {\n    case 1: {\n        if (u) {\n            set();\n            break;\n        }\n        /* else... */\n    }  /* FALLTHROUGH */\n    case 2:\n        y();\n    }\n}\n";
    check(
        input,
        &["--style=horstmann"],
        "void f()\n{   switch (w)\n    {   case 1:\n        {   if (u)\n            {   set();\n                break;\n            }\n            /* else... */\n            }  /* FALLTHROUGH */\n        case 2:\n            y();\n    }\n}\n",
    );
}

#[test]
fn empty_lines_in_an_indented_preprocessor_block_take_its_indent() {
    let input = "#ifndef X\n#define X\n#ifdef W\n#include <a.h>\n\nint f(int);\n\n\nint g(int);\n#endif\n\nint h;\n#endif\n#if A || \\\n  B\n\nint k;\n#endif\n";
    check(
        input,
        &["--fill-empty-lines", "--indent-preproc-block"],
        "#ifndef X\n    #define X\n    #ifdef W\n        #include <a.h>\n        \n        int f(int);\n        \n        \n        int g(int);\n    #endif\n    \n    int h;\n#endif\n#if A || \\\n    B\n    \n    int k;\n#endif\n",
    );
}

#[test]
fn a_block_after_a_split_condition_in_a_nested_broken_else_if_keeps_one_level() {
    let input = "void f()\n{\n    if (a) {\n        x();\n    }\n    else if (b) {\n        if (c)\n            y();\n        else if (d &&\n                 e) {\n            z();\n        }\n    }\n}\n";
    check(
        input,
        &["--style=allman", "--break-elseifs"],
        "void f()\n{\n    if (a)\n    {\n        x();\n    }\n    else\n        if (b)\n        {\n            if (c)\n                y();\n            else\n                if (d &&\n                        e)\n                {\n                    z();\n                }\n        }\n}\n",
    );
}

#[test]
fn a_sign_leading_an_initializer_row_after_its_brace_stays_unary() {
    let input =
        "int a[] = {\n  -1, 2\n};\nvoid f()\n{\n    int c[] = {\n        -x, 2\n    };\n}\n";
    let expected =
        "int a[] = {\n    -1, 2\n};\nvoid f()\n{\n    int c[] = {\n        -x, 2\n    };\n}\n";
    check(input, &["--pad-oper"], expected);
}

#[test]
fn a_const_pointer_cast_in_a_braced_body_aligns_to_the_name() {
    let input = "void f()\n{\n    if (n) a = (const char * const *)b;\n    if (n) a = (char * const *)b;\n    if (n) a = (char *)b;\n    if (n) a = (char **)b;\n}\n";
    check(
        input,
        &["--add-braces", "--align-pointer=name"],
        "void f()\n{\n    if (n) {\n        a = (const char *const *)b;\n    }\n    if (n) {\n        a = (char *const *)b;\n    }\n    if (n) {\n        a = (char *)b;\n    }\n    if (n) {\n        a = (char **)b;\n    }\n}\n",
    );
}

#[test]
fn a_const_pointer_cast_in_a_braced_body_aligns_to_the_type() {
    let input = "void f()\n{\n    if (n) a = (const char * const *)b;\n    if (n) a = (char * const *)b;\n    if (n) a = (char *)b;\n    if (n) a = (char **)b;\n}\n";
    check(
        input,
        &["--add-braces", "--align-pointer=type"],
        "void f()\n{\n    if (n) {\n        a = (const char* const*)b;\n    }\n    if (n) {\n        a = (char* const*)b;\n    }\n    if (n) {\n        a = (char*)b;\n    }\n    if (n) {\n        a = (char**)b;\n    }\n}\n",
    );
}

#[test]
fn a_function_pointer_after_a_calling_convention_is_a_declarator_with_name() {
    let input = "typedef int (WSAAPI* LPFN_WSARECV)\n    (int s);\ntypedef int (WSAAPI *LPFN_X)(int s);\ntypedef int (CALLBACK * F)(int);\nstatic int (WSAAPI * G)(int s);\nvoid f()\n{\n    foo(a * b)(c);\n}\n";
    check(
        input,
        &["--align-pointer=name"],
        "typedef int (WSAAPI *LPFN_WSARECV)\n(int s);\ntypedef int (WSAAPI *LPFN_X)(int s);\ntypedef int (CALLBACK *F)(int);\nstatic int (WSAAPI *G)(int s);\nvoid f()\n{\n    foo(a * b)(c);\n}\n",
    );
}

#[test]
fn a_function_pointer_after_a_calling_convention_is_a_declarator_with_padded_operators() {
    let input = "typedef int (WSAAPI* LPFN_WSARECV)\n    (int s);\ntypedef int (WSAAPI *LPFN_X)(int s);\ntypedef int (CALLBACK * F)(int);\nstatic int (WSAAPI * G)(int s);\nvoid f()\n{\n    foo(a * b)(c);\n}\n";
    check(
        input,
        &["--pad-oper"],
        "typedef int (WSAAPI* LPFN_WSARECV)\n(int s);\ntypedef int (WSAAPI *LPFN_X)(int s);\ntypedef int (CALLBACK * F)(int);\nstatic int (WSAAPI * G)(int s);\nvoid f()\n{\n    foo(a * b)(c);\n}\n",
    );
}

#[test]
fn a_closing_brace_row_in_a_define_takes_its_level_without_conditional_indent() {
    let input = "#define F \\\n  int a;     \\\n  union {    \\\n    void* r; \\\n    int w;   \\\n  };\n\nint x;\n";
    check(
        input,
        &["--indent-preproc-define", "--min-conditional-indent=0"],
        "#define F \\\n    int a;     \\\n    union {    \\\n        void* r; \\\n        int w;   \\\n    };\n\nint x;\n",
    );
}

#[test]
fn a_declaration_in_a_conditional_extern_c_block_keeps_its_return_type() {
    let input = "#ifdef __cplusplus\nextern \"C\" {\n#endif\n\nint f(int a);\nvoid g(void);\n\n#ifdef __cplusplus\n}\n#endif\n";
    check(
        input,
        &["--break-return-type-decl"],
        "#ifdef __cplusplus\nextern \"C\" {\n#endif\n\nint f(int a);\nvoid g(void);\n\n#ifdef __cplusplus\n}\n#endif\n",
    );
}

#[test]
fn functions_in_an_extern_c_block_keep_their_return_types() {
    let input = "extern \"C\" {\nint f(int a)\n{\n    return 0;\n}\nint\ng(void);\n}\nnamespace n {\nint h(int a);\n}\n";
    check(
        input,
        &["--break-return-type", "--attach-return-type-decl"],
        "extern \"C\" {\n    int f(int a)\n    {\n        return 0;\n    }\n    int\n    g(void);\n}\nnamespace n {\nint h(int a);\n}\n",
    );
}

#[test]
fn macro_body_statements_indent_with_tabs_and_their_continuations_align_in_spaces() {
    let input = "#define X(a) {   \\\n  if (a) {        \\\n    foo(a,          \\\n        b);         \\\n  }               \\\n}\n#define Y(a)     \\\n  foo(a,          \\\n      b)\n#define Z(a)     \\\n  do {            \\\n    x = a +       \\\n        b;        \\\n  } while (0)\n#define for_each(j, t)\t\t\\\n\tfor (j = 0, t = b[j];\t\\\n\t     t;\t\t\\\n\t     j++, t = b[j])\n";
    check(
        input,
        &["--indent=tab=4", "--indent-preproc-define"],
        "#define X(a) {   \\\n\t\tif (a) {        \\\n\t\t\tfoo(a,          \\\n\t\t\t    b);         \\\n\t\t}               \\\n\t}\n#define Y(a)     \\\n\tfoo(a,          \\\n\t    b)\n#define Z(a)     \\\n\tdo {            \\\n\t\tx = a +       \\\n\t\t    b;        \\\n\t} while (0)\n#define for_each(j, t)\t\t\\\n\tfor (j = 0, t = b[j];\t\\\n\t        t;\t\t\\\n\t        j++, t = b[j])\n",
    );
}

#[test]
fn a_macro_row_closing_its_block_after_code_keeps_the_continuation_column() {
    let input = "#define G(df, buf) {    \\\n  uInt s=D(df, 0);      \\\n  (buf)[0]=A[s&0x3ff]   \\\n          +B[s&0x3ff]   \\\n          +M[C[s>>26]];}\n";
    check(
        input,
        &["--indent-preproc-define"],
        "#define G(df, buf) {    \\\n        uInt s=D(df, 0);      \\\n        (buf)[0]=A[s&0x3ff]   \\\n                 +B[s&0x3ff]   \\\n                 +M[C[s>>26]];}\n",
    );
}

#[test]
fn a_compound_literal_pads_its_closing_brace_in_pico() {
    let input = "void f()\n{\n    *out = (T) {0};\n    g(c, (struct timeval) { .tv_sec = 1, .tv_usec = 0});\n    g(c, (struct timeval) { .tv_sec = 1, .tv_usec = 0 });\n    int a[] = {1, 2};\n}\n";
    check(
        input,
        &["--style=pico"],
        "void f()\n{   *out = (T) {0 };\n    g(c, (struct timeval) { .tv_sec = 1, .tv_usec = 0 });\n    g(c, (struct timeval) { .tv_sec = 1, .tv_usec = 0 });\n    int a[] = {1, 2 }; }\n",
    );
}

#[test]
fn a_comment_after_a_closing_brace_moved_up_follows_it_by_one_column_less_in_pico() {
    let input = "void f()\n{\n    if (b) {\n        x();\n        return 1;\n    }  /* else not */\n    if (a) { i++; j++; }  /* skip */\n    y();\n}\nstruct s {\n    int x;\n};  /* c */\n";
    check(
        input,
        &["--style=pico"],
        "void f()\n{   if (b)\n    {   x();\n        return 1; } /* else not */\n    if (a) { i++; j++; }  /* skip */\n    y(); }\nstruct s\n{   int x; }; /* c */\n",
    );
}

#[test]
fn a_comment_after_a_closing_brace_moved_up_follows_it_by_one_column_less_in_lisp() {
    let input = "void f()\n{\n    if (b) {\n        x();\n        return 1;\n    }  /* else not */\n    if (a) { i++; j++; }  /* skip */\n    y();\n}\nstruct s {\n    int x;\n};  /* c */\n";
    check(
        input,
        &["--style=lisp"],
        "void f() {\n    if (b) {\n        x();\n        return 1; } /* else not */\n    if (a) {\n        i++;    /* skip */\n        j++; }\n    y(); }\nstruct s {\n    int x; }; /* c */\n",
    );
}

#[test]
fn unpadding_keeps_the_last_whitespace_character_before_a_declarator_paren() {
    let input = "struct s {\n    void\t(*cb)(int);\n    void  (*db)(int);\n    int\t*arg;\n};\nvoid f()\n{\n    x = g\t(a);\n    if\t(a) b();\n}\nstruct t {\n    void \t(*cb)(int);\n    void\t (*db)(int);\n};\n";
    check(
        input,
        &["--unpad-paren"],
        "struct s {\n    void\t(*cb)(int);\n    void (*db)(int);\n    int\t*arg;\n};\nvoid f()\n{\n    x = g(a);\n    if(a) b();\n}\nstruct t {\n    void\t(*cb)(int);\n    void (*db)(int);\n};\n",
    );
}

#[test]
fn a_case_label_after_a_continued_directive_follows_the_label_before_it() {
    let input = "static bool g(int rc)\n{\n  switch(rc) {\n#ifdef A\n  case A:\n#endif\n#if defined(B) && \\\n  (!defined(A) || (B != A))\n  case B:\n#endif\n    return TRUE;\n  default:\n    return FALSE;\n  }\n}\n";
    check(
        input,
        &["--break-blocks=all"],
        "static bool g(int rc)\n{\n    switch(rc) {\n#ifdef A\n\n    case A:\n#endif\n#if defined(B) && \\\n  (!defined(A) || (B != A))\n    case B:\n#endif\n        return TRUE;\n\n    default:\n        return FALSE;\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_a_continued_conditional_takes_the_block_indent() {
    let input = "#include <signal.h>\n\n#if defined(A) && \\\n    defined(B)\n/* set a tiny limit */\n#define T 50\n#endif\n";
    check(
        input,
        &["--indent-preproc-block"],
        "#include <signal.h>\n\n#if defined(A) && \\\n    defined(B)\n    /* set a tiny limit */\n    #define T 50\n#endif\n",
    );
}

#[test]
fn a_macro_header_condition_indents_after_its_paren_without_conditional_indent() {
    let input = "#define SLIST_FOREACH(var, head, field)\t\t\t\t\\\n\tfor((var) = SLIST_FIRST(head);\t\t\t\t\\\n\t    (var) != SLIST_END(head);\t\t\t\t\t\\\n\t    (var) = SLIST_NEXT(var, field))\n#define W(a) \\\n    if (a &&  \\\n        b)    \\\n        y()\n";
    check(
        input,
        &["--indent-after-parens", "--indent-preproc-define"],
        "#define SLIST_FOREACH(var, head, field)\t\t\t\t\\\n    for((var) = SLIST_FIRST(head);\t\t\t\t\\\n        (var) != SLIST_END(head);\t\t\t\t\t\\\n        (var) = SLIST_NEXT(var, field))\n#define W(a) \\\n    if (a &&  \\\n        b)    \\\n        y()\n",
    );
}

#[test]
fn conditionals_in_a_header_condition_and_their_continued_lines_follow_astyle() {
    let input = "void f()\n{\n    if ((err == A\n#if (W)\n         || err == B\n#endif\n        )) {\n        x();\n    }\n    const char *s[] = {\n        \"a\"\n#if !defined(X) ||  \\\n  (defined(Y) ||       \\\n   defined(Z))\n        \"b\"\n#endif\n    };\n}\n";
    check(
        input,
        &["--indent-preproc-cond"],
        "void f()\n{\n    if ((err == A\n        #if (W)\n            || err == B\n        #endif\n        )) {\n        x();\n    }\n    const char *s[] = {\n        \"a\"\n        #if !defined(X) ||  \\\n        (defined(Y) ||       \\\n         defined(Z))\n        \"b\"\n        #endif\n    };\n}\n",
    );
}

#[test]
fn a_header_alternative_after_else_directive_takes_no_blank_line() {
    let input = "void f()\n{\n  if (a)\n    abort();\n\n#if defined(M)\n  if (b(1))\n#else\n  if (b(2))\n#endif\n    abort();\n\n  x();\n}\n";
    check(
        input,
        &["--break-blocks"],
        "void f()\n{\n    if (a)\n        abort();\n\n#if defined(M)\n\n    if (b(1))\n#else\n    if (b(2))\n#endif\n        abort();\n\n    x();\n}\n",
    );
}

#[test]
fn a_tagged_type_star_before_a_declarator_group_stays_as_written() {
    let input = "void f()\n{\n    typedef const struct S *(*letterfunc)(const char *ptr);\n    typedef int *(*g)(int);\n}\ntypedef char *(*h)(int);\nvoid g()\n{\n    struct S *x;\n    const struct S *y = 0;\n    struct S *(*fp)(int);\n}\n";
    check(
        input,
        &["--align-pointer=type"],
        "void f()\n{\n    typedef const struct S *(*letterfunc)(const char* ptr);\n    typedef int* (*g)(int);\n}\ntypedef char* (*h)(int);\nvoid g()\n{\n    struct S* x;\n    const struct S* y = 0;\n    struct S *(*fp)(int);\n}\n",
    );
}

#[test]
fn a_star_before_a_declarator_group_moves_its_gap_or_stays_by_context_with_type_alignment() {
    let input = "struct T {\n    struct S\t*(*fp)(int);\n};\nvoid f()\n{\n    struct S\t*(*gp)(int);\n}\nstruct S\t*(*hp)(int);\nvoid k(struct S\t*(*a)(int));\ntypedef int  *(*g)(int);\ntypedef int\t*(*h)(int);\nvoid f()\n{\n    int  *(*k)(int);\n}\n";
    check(
        input,
        &["--align-pointer=type"],
        "struct T {\n    struct S*\t(*fp)(int);\n};\nvoid f()\n{\n    struct S\t*(*gp)(int);\n}\nstruct S*\t(*hp)(int);\nvoid k(struct S\t*(*a)(int));\ntypedef int*  (*g)(int);\ntypedef int*\t(*h)(int);\nvoid f()\n{\n    int*  (*k)(int);\n}\n",
    );
}

#[test]
fn a_star_before_a_declarator_group_moves_its_gap_or_stays_by_context_with_middle_alignment() {
    let input = "struct T {\n    struct S\t*(*fp)(int);\n};\nvoid f()\n{\n    struct S\t*(*gp)(int);\n}\nstruct S\t*(*hp)(int);\nvoid k(struct S\t*(*a)(int));\ntypedef int  *(*g)(int);\ntypedef int\t*(*h)(int);\nvoid f()\n{\n    int  *(*k)(int);\n}\n";
    check(
        input,
        &["--align-pointer=middle"],
        "struct T {\n    struct S\t* (*fp)(int);\n};\nvoid f()\n{\n    struct S\t*(*gp)(int);\n}\nstruct S\t* (*hp)(int);\nvoid k(struct S\t*(*a)(int));\ntypedef int * (*g)(int);\ntypedef int\t* (*h)(int);\nvoid f()\n{\n    int * (*k)(int);\n}\n",
    );
}

#[test]
fn a_label_kept_after_a_case_block_closer_opens_its_body_with_allman() {
    let input = "void f()\n{\n    switch (a) {\n    case 1: {\n        x();\n    } default:\n        not_reached();\n    }\n}\n";
    check(
        input,
        &["--style=allman", "--keep-one-line-statements"],
        "void f()\n{\n    switch (a)\n    {\n    case 1:\n    {\n        x();\n    } default:\n        not_reached();\n    }\n}\n",
    );
}

#[test]
fn a_label_kept_after_a_case_block_closer_opens_its_body_with_indented_switches() {
    let input = "void f()\n{\n    switch (a) {\n    case 1: {\n        x();\n    } default:\n        not_reached();\n    }\n}\n";
    check(
        input,
        &[
            "--style=kr",
            "--keep-one-line-statements",
            "--indent-switches",
        ],
        "void f()\n{\n    switch (a) {\n        case 1: {\n            x();\n        } default:\n            not_reached();\n    }\n}\n",
    );
}

#[test]
fn an_added_block_after_a_broken_else_if_in_a_case_takes_its_level() {
    let input = "void f()\n{\n    switch (s) {\n    case 1:\n        if (t) {\n            if (x)\n                y = 1;\n            else if (n)\n                return (-1);\n        }\n        break;\n    }\n}\n";
    check(
        input,
        &[
            "--style=whitesmith",
            "--break-elseifs",
            "--add-one-line-braces",
        ],
        "void f()\n    {\n    switch (s)\n        {\n        case 1:\n            if (t)\n                {\n                if (x)\n                    { y = 1; }\n                else\n                    if (n)\n                        { return (-1); }\n                }\n            break;\n        }\n    }\n",
    );
}

#[test]
fn an_initializer_brace_stays_off_a_line_it_would_take_past_the_maximum_length() {
    let input = "static ngx_command_t  ngx_stream_upstream_hash_commands[] =\n{\n    { ngx_string(\"hash\"),\n      0 },\n};\n";
    check(
        input,
        &["--style=google", "--max-code-length=60"],
        "static ngx_command_t  ngx_stream_upstream_hash_commands[] =\n{\n    {\n        ngx_string(\"hash\"),\n        0\n    },\n};\n",
    );
}

#[test]
fn a_conditional_in_a_header_condition_stands_a_level_before_its_column() {
    let input = "void f()\n{\n    if (r == 0)\n        x();\n    else if (errno == E\n#if defined(O)\n             || errno == B\n#endif\n            )\n        y();\n}\nvoid g()\n{\n    if(\n#if A\n        x &&\n#endif\n        y) {\n        z();\n    }\n}\n";
    check(
        input,
        &["--style=mozilla", "--indent-preproc-cond"],
        "void f()\n{\n    if (r == 0)\n        x();\n    else if (errno == E\n         #if defined(O)\n             || errno == B\n         #endif\n            )\n        y();\n}\nvoid g()\n{\n    if(\n    #if A\n        x &&\n    #endif\n        y) {\n        z();\n    }\n}\n",
    );
}

#[test]
fn a_logical_operand_in_a_condition_indents_after_the_paren_in_a_branch_block() {
    let input = "int f(void)\n{\n#ifdef A\n    x = 1;\n#else\n    {\n        if((h != I) &&\n            G(h, &c)) {\n            x();\n        }\n    }\n#endif\n}\n";
    check(
        input,
        &["--indent-after-parens"],
        "int f(void)\n{\n#ifdef A\n    x = 1;\n#else\n    {\n        if((h != I) &&\n            G(h, &c)) {\n            x();\n        }\n    }\n#endif\n}\n",
    );
}

#[test]
fn a_comment_after_a_broken_brace_keeps_its_column_with_padded_parens_in_allman() {
    let input = "int f()\n{\n    if (max == 0) {                     /* no symbols */\n        x = 1;\n        return 0;\n    }\n    if (left > 0 && (type == C || max != 1))\n        return -1;                      /* incomplete set */\n    return 0;\n}\n";
    check(
        input,
        &["--style=allman", "--pad-paren-in"],
        "int f()\n{\n    if ( max == 0 )                     /* no symbols */\n    {\n        x = 1;\n        return 0;\n    }\n    if ( left > 0 && ( type == C || max != 1 ) )\n        return -1;                      /* incomplete set */\n    return 0;\n}\n",
    );
}

#[test]
fn a_comment_after_a_broken_brace_keeps_its_column_with_padded_parens_in_pico() {
    let input = "int f()\n{\n    if (max == 0) {                     /* no symbols */\n        x = 1;\n        return 0;\n    }\n    if (left > 0 && (type == C || max != 1))\n        return -1;                      /* incomplete set */\n    return 0;\n}\n";
    check(
        input,
        &["--style=pico", "--pad-paren-out"],
        "int f()\n{   if (max == 0)                       /* no symbols */\n    {   x = 1;\n        return 0; }\n    if (left > 0 && (type == C || max != 1) )\n        return -1;                      /* incomplete set */\n    return 0; }\n",
    );
}

#[test]
fn an_added_closer_goes_before_the_statement_comment_with_its_gap_in_pico() {
    let input = "int f()\n{\n    if (a)\n        return -1;                      /* incomplete set */\n    return 0;\n}\n";
    check(
        input,
        &["--style=pico", "--add-braces"],
        "int f()\n{   if (a)\n    {   return -1; }                      /* incomplete set */\n    return 0; }\n",
    );
}

#[test]
fn an_added_closer_goes_before_the_statement_comment_with_its_gap_in_lisp() {
    let input = "int f()\n{\n    if (a)\n        return -1;                      /* incomplete set */\n    return 0;\n}\n";
    check(
        input,
        &["--style=lisp", "--add-braces"],
        "int f() {\n    if (a) {\n        return -1; }                      /* incomplete set */\n    return 0; }\n",
    );
}

#[test]
fn macro_rows_inside_a_directive_brace_and_filled_block_lines_take_tabs() {
    let input = "#define RTREE_CTX_INITIALIZER {{A(RTREE_CTX_NCACHE)}, \\\n\t\t{B(RTREE_CTX_NCACHE_L2)}}\n\n#ifdef X\n#include <a.h>\n\nint f(int);\n#endif\n\nint y;\n";
    check(
        input,
        &[
            "--indent=tab=8",
            "--indent-preproc-define",
            "--fill-empty-lines",
            "--indent-preproc-block",
        ],
        "#define RTREE_CTX_INITIALIZER {{A(RTREE_CTX_NCACHE)}, \\\n\t\t{B(RTREE_CTX_NCACHE_L2)}}\n\n#ifdef X\n\t#include <a.h>\n\t\n\tint f(int);\n#endif\n\nint y;\n",
    );
}

#[test]
fn a_block_without_code_stays_on_its_header_line_in_pico() {
    let input = "void f()\n{\n    if (r->postponed) {\n        for (pr = r->postponed; pr->next; pr = pr->next) { /* void */ }\n        x();\n    }\n    for (;;) { /* void */ }\n}\nvoid g()\n{\n    for (;;) { x(); }\n    for (;;) { }\n    if (a) { /* c */ }\n}\n";
    check(
        input,
        &["--style=pico", "--break-one-line-headers"],
        "void f()\n{   if (r->postponed)\n    {   for (pr = r->postponed; pr->next; pr = pr->next) { /* void */ }\n        x(); }\n    for (;;) { /* void */ } }\nvoid g()\n{   for (;;)\n    {   x(); }\n    for (;;) { }\n    if (a) { /* c */ } }\n",
    );
}

#[test]
fn named_width_bit_fields_in_a_class_keep_their_colon_spacing() {
    let input = "class C {\npublic:\n    int x : WIDTH;\n    int z : 3;\n    int w:4;\nprivate:\n    int y;\n};\nstruct E {\npublic:\n    int x : WIDTH;\n};\n";
    let expected = "class C\n{\npublic:\n    int x : WIDTH;\n    int z : 3;\n    int w:4;\nprivate:\n    int y;\n};\nstruct E {\npublic:\n    int x : WIDTH;\n};\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn named_width_bit_fields_in_a_class_keep_their_colon_spacing_padded() {
    let input = "class C {\npublic:\n    int x : WIDTH;\n    int z:3;\n};\n";
    let expected = "class C\n{\npublic:\n    int x : WIDTH;\n    int z: 3;\n};\n";
    check(input, &["--style=kr", "--pad-oper"], expected);
    check(expected, &["--style=kr", "--pad-oper"], expected);
}

#[test]
fn subscript_rows_continue_past_their_bracket() {
    let input = "void f(void)\n{\n    here = state->lencode[last.val +\n        (BITS(last.bits + last.op) >> last.bits)];\n    x = s->a[b +\n        c];\n    y = get()[idx +\n        1];\n    g(a[b.c +\n        c]);\n}\n";
    let expected = "void f(void)\n{\n    here = state->lencode[last.val +\n                          (BITS(last.bits + last.op) >> last.bits)];\n    x = s->a[b +\n             c];\n    y = get()[idx +\n              1];\n    g(a[b.c +\n        c]);\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn shift_rows_in_a_subscript_continue_past_its_bracket() {
    let input = "int f(void)\n{\n    x = tab[size <<\n        MIN];\n    tab[size >>\n        MIN] = 1;\n    return tab[(size + 1)\n        >> MIN];\n}\n";
    let expected = "int f(void)\n{\n    x = tab[size <<\n            MIN];\n    tab[size >>\n        MIN] = 1;\n    return tab[(size + 1)\n               >> MIN];\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn shift_row_in_a_returned_subscript_continues_past_its_bracket_under_max_code_length() {
    let input = "int f(size_t size)\n{\n\treturn sz_size2index_tab[(size + (ZU(1) << SC_LG_TINY_MIN) - 1)\n\t    >> SC_LG_TINY_MIN];\n}\n";
    let expected = "int f(size_t size)\n{\n    return sz_size2index_tab[(size + (ZU(1) << SC_LG_TINY_MIN) - 1)\n                             >> SC_LG_TINY_MIN];\n}\n";
    check(input, &["--style=kr", "--max-code-length=109"], expected);
    check(expected, &["--style=kr", "--max-code-length=109"], expected);
}

#[test]
fn defer_statements_take_no_added_braces() {
    let input = "void f(void)\n{\n    void *p = malloc(10);\n    defer free(p);\n    defer { free(p); }\n    defer {\n        free(p);\n        close(fd);\n    }\n}\n";
    let expected = "void f(void)\n{\n    void *p = malloc(10);\n    defer free(p);\n    defer { free(p); }\n    defer {\n        free(p);\n        close(fd);\n    }\n}\n";
    check(
        input,
        &[
            "--style=kr",
            "--add-braces",
            "--break-one-line-headers",
            "--pad-header",
        ],
        expected,
    );
    check(
        expected,
        &[
            "--style=kr",
            "--add-braces",
            "--break-one-line-headers",
            "--pad-header",
        ],
        expected,
    );
}

#[test]
fn sibling_brace_rows_of_a_compound_literal_stand_with_the_first() {
    let input = "static S s = {\n    .args = (Arg[]){\n        {\n            .name = \"key\",\n        },\n        {\n            .name = \"x\",\n        },\n        {0}\n    },\n    .b = 1,\n};\nvoid f(void)\n{\n    g((Arg[]){\n        {\n            .name = \"key\",\n        },\n        {\n            .name = \"x\",\n        },\n    });\n}\n";
    let expected = "static S s = {\n    .args = (Arg[]){\n        {\n            .name = \"key\",\n        },\n        {\n            .name = \"x\",\n        },\n        {0}\n    },\n    .b = 1,\n};\nvoid f(void)\n{\n    g((Arg[]) {\n        {\n            .name = \"key\",\n        },\n        {\n            .name = \"x\",\n        },\n    });\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn compound_literal_in_a_run_in_element_indents_from_its_row() {
    let input = "void f(void)\n{\n    A args[] = {\n        { .name = \"key\", .t = 0 },\n        { .name = \"format\", .subargs = (A[]) {\n                { .name = \"ele\" },\n                { .name = NULL }\n            }\n        },\n        { .name = \"v\" },\n    };\n}\n";
    let expected = "void f(void)\n{\n    A args[] = {\n        { .name = \"key\", .t = 0 },\n        {\n            .name = \"format\", .subargs = (A[]) {\n                { .name = \"ele\" },\n                { .name = NULL }\n            }\n        },\n        { .name = \"v\" },\n    };\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn cast_compound_literals_stay_on_one_line() {
    let input = "void f(void)\n{\n    p = (int *)(int[]){ 1, 2, 3 };\n    p = (int[]){ 1, 2, 3 };\n    g((int[]){ 1, 2 });\n    h(&(struct timeval){ .tv_sec = 1 });\n}\n";
    let expected = "void f(void)\n{\n    p = (int *)(int[]) { 1, 2, 3 };\n    p = (int[]) { 1, 2, 3 };\n    g((int[]) { 1, 2 });\n    h(&(struct timeval) { .tv_sec = 1 });\n}\n";
    check(input, &["--style=allman"], expected);
    check(expected, &["--style=allman"], expected);
}

// A cast after an operator of an expression is a value's, not a
// declarator's; a function returning a pointer keeps its body.
#[test]
fn cast_compound_literals_after_operators_stay_on_one_line() {
    let input = "void f(void)\n{\n    x = a * (int *)(int[]){1};\n    z = *(int *)(int[]){1};\n    f(a & (int *)(int[]){1});\n    return a ^ (T)(U){1};\n}\nint *(g)(int a[]) {\n    return 0;\n}\n";
    let expected = "void f(void)\n{\n    x = a * (int *)(int[]) {1};\n    z = *(int *)(int[]) {1};\n    f(a & (int *)(int[]) {1});\n    return a ^ (T)(U) {1};\n}\nint *(g)(int a[])\n{\n    return 0;\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn compound_literal_closing_brace_leaves_the_last_row() {
    let input = "void f(void)\n{\n    p = (int[]){\n        1, 2, 3 };\n    g((int[]){\n        1, 2, 3 });\n}\n";
    let expected = "void f(void)\n{\n    p = (int[]) {\n        1, 2, 3\n    };\n    g((int[]) {\n        1, 2, 3\n    });\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn statement_compound_literal_rows_after_a_run_in_first_element_stand_a_level_in() {
    let input = "void f(void)\n{\n    q = (int[]){ 1, 2,\n        3 };\n    return (struct p){ .a = 1,\n        .b = 2 };\n    g((int[]){ 1,\n        2 }, 3);\n}\n";
    let expected = "void f(void)\n{\n    q = (int[]) { 1, 2,\n        3\n    };\n    return (struct p) { .a = 1,\n        .b = 2\n    };\n    g((int[]) { 1,\n                2 }, 3);\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn broken_compound_literal_brace_attaches_in_attaching_styles() {
    let input = "void f(void)\n{\n    q = (struct point)\n    {\n        .x = 1,\n    };\n    g((struct point)\n    {\n        .x = 1,\n    });\n}\n";
    let expected = "void f(void)\n{\n    q = (struct point) {\n        .x = 1,\n    };\n    g((struct point) {\n        .x = 1,\n    });\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn compound_literal_nested_in_an_initializer_keeps_its_brace_attached() {
    let input = "static S s = {\n    .args = (Arg[]) {\n        1,\n        {0},\n        {\n            3\n        },\n    },\n};\n";
    let expected = "static S s = {\n    .args = (Arg[]) {\n        1,\n        {0},\n        {\n            3\n        },\n    },\n};\n";
    check(input, &["--style=allman"], expected);
    check(expected, &["--style=allman"], expected);
}

#[test]
fn whitesmith_indents_brace_rows_of_a_compound_literal() {
    let input = "void f(void)\n{\n    x = g(a, (int[][1]) {\n        1,\n        {0},\n        {\n            3\n        },\n        {\n            3\n        },\n    });\n}\n";
    let expected = "void f(void)\n    {\n    x = g(a, (int[][1])\n        {\n        1,\n            {0},\n            {\n            3\n            },\n            {\n            3\n            },\n        });\n    }\n";
    check(input, &["--style=whitesmith"], expected);
    check(expected, &["--style=whitesmith"], expected);
}

#[test]
fn vtk_indents_brace_rows_of_a_compound_literal() {
    let input = "void f(void)\n{\n    x = g(a, (int[][1]) {\n        1,\n        {0},\n        {\n            3\n        },\n        {\n            3\n        },\n    });\n}\n";
    let expected = "void f(void)\n{\n    x = g(a, (int[][1])\n        {\n        1,\n            {0},\n            {\n            3\n            },\n            {\n            3\n            },\n        });\n}\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
}

#[test]
fn compound_literal_field_takes_no_column_from_an_earlier_initializer() {
    let input = "static S s = {\n    .h = 1,\n};\nvoid f(void)\n{\n    T info = {\n        .h = (H[]){\n            {0}\n        },\n    };\n}\n";
    let expected = "static S s = {\n    .h = 1,\n};\nvoid f(void)\n{\n    T info = {\n        .h = (H[]){\n            {0}\n        },\n    };\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn block_comment_among_compound_literal_elements_stands_with_them() {
    let input = "static S s = {\n    .history = (H[]){\n        /* note a\n         * b */\n        {\"5.0\", \"x\"},\n        {0}\n    },\n};\nvoid f(void)\n{\n    g((H[]){\n        /* a\n         * b */\n        {1},\n    });\n}\n";
    let expected = "static S s = {\n    .history = (H[]){\n        /* note a\n         * b */\n        {\"5.0\", \"x\"},\n        {0}\n    },\n};\nvoid f(void)\n{\n    g((H[]) {\n        /* a\n         * b */\n        {1},\n    });\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn element_after_a_trailing_block_comment_stands_with_the_elements() {
    let input = "void f(void)\n{\n    y = { 1, /* a\n                   b */\n        2 };\n    int z[] = { 1, /* a\n                   b */\n        2 };\n}\n";
    let expected = "void f(void)\n{\n    y = { 1, /* a\n                   b */\n          2\n        };\n    int z[] = { 1, /* a\n                   b */\n                2\n              };\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn nested_compound_literal_fields_stand_a_level_in_whitesmith() {
    let input = "void f(void)\n{\n    T x = {\n        .a = 1,\n        .t = (struct u) {\n            .user = 1,\n            .nice = 2,\n        },\n    };\n}\nvoid g(void)\n{\n    (*ci)[i++] = (uv_cpu_info_t) {\n      .speed     = c->freq / 1000,\n      .cpu_times = (struct uv_cpu_times_s) {\n        .user = 10 * c->user,\n        .irq  = 10 * c->irq,\n      },\n    };\n}\n";
    let expected = "void f(void)\n    {\n    T x =\n        {\n        .a = 1,\n        .t = (struct u) {\n            .user = 1,\n            .nice = 2,\n            },\n        };\n    }\nvoid g(void)\n    {\n    (*ci)[i++] = (uv_cpu_info_t)\n        {\n        .speed     = c->freq / 1000,\n        .cpu_times = (struct uv_cpu_times_s) {\n            .user = 10 * c->user,\n            .irq  = 10 * c->irq,\n            },\n        };\n    }\n";
    check(input, &["--style=whitesmith"], expected);
    check(expected, &["--style=whitesmith"], expected);
}

#[test]
fn nested_compound_literal_fields_stand_a_level_in_allman() {
    let input = "void f(void)\n{\n    T x = {\n        .a = 1,\n        .t = (struct u) {\n            .user = 1,\n            .nice = 2,\n        },\n    };\n}\n";
    let expected = "void f(void)\n{\n    T x =\n    {\n        .a = 1,\n        .t = (struct u) {\n            .user = 1,\n            .nice = 2,\n        },\n    };\n}\n";
    check(input, &["--style=allman"], expected);
    check(expected, &["--style=allman"], expected);
}

#[test]
fn nested_compound_literal_fields_stand_a_level_in_vtk() {
    let input = "void f(void)\n{\n    (*ci)[i++] = (uv_cpu_info_t) {\n      .speed     = c->freq / 1000,\n      .cpu_times = (struct uv_cpu_times_s) {\n        .user = 10 * c->user,\n        .irq  = 10 * c->irq,\n      },\n    };\n}\n";
    let expected = "void f(void)\n{\n    (*ci)[i++] = (uv_cpu_info_t)\n        {\n        .speed     = c->freq / 1000,\n        .cpu_times = (struct uv_cpu_times_s) {\n            .user = 10 * c->user,\n            .irq  = 10 * c->irq,\n            },\n        };\n}\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
}

#[test]
fn compound_literal_nested_in_an_initializer_keeps_its_brace_gap() {
    let input = "static S s = {\n    .a ={\n        1,\n    },\n    .b = (T){\n        1,\n    },\n    .c = (T)  {\n        1,\n    },\n};\nvoid f(void)\n{\n    x = (T){\n        1,\n    };\n}\n";
    let expected = "static S s = {\n    .a ={\n        1,\n    },\n    .b = (T){\n        1,\n    },\n    .c = (T)  {\n        1,\n    },\n};\nvoid f(void)\n{\n    x = (T) {\n        1,\n    };\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn whitesmith_last_brace_row_of_a_compound_literal_stands_with_the_rows() {
    let input = "static S s = {\n    .h = (H[]){\n        {1, 2},\n        {0}\n    },\n};\n";
    let expected = "static S s = {\n    .h = (H[]){\n            {1, 2},\n            {0}\n        },\n    };\n";
    check(input, &["--style=whitesmith"], expected);
    check(expected, &["--style=whitesmith"], expected);
}
#[test]
fn closing_bracket_row_stands_a_level_before_the_rows_its_bracket_opened() {
    let input = "static unsigned char mem[\n  1024 * 4 /* a\n              b */\n] = {0};\nvoid f(void)\n{\n    x = tab[\n        i +\n        1\n    ];\n}\n";
    let expected = "static unsigned char mem[\n    1024 * 4 /* a\n              b */\n] = {0};\nvoid f(void)\n{\n    x = tab[\n            i +\n            1\n        ];\n}\n";
    check(input, &["--style=kr"], expected);
    check(expected, &["--style=kr"], expected);
}

#[test]
fn a_block_comment_after_an_else_makes_the_following_header_its_body() {
    let input = "void f(void){\n  if( a ){\n    x();\n  }else\n  /* dump\n  */\n  if( b ){\n    while( c ){\n      switch( op ){\n        case 1: {\n          int i;\n          y();\n          break;\n        }\n      }\n    }\n  }\n}\n";
    check(
        input,
        &["--style=pico"],
        "void f(void)\n{   if( a )\n    {   x(); }\n    else\n        /* dump\n        */\n        if( b )\n        {   while( c )\n            {   switch( op )\n                {   case 1:\n                    {   int i;\n                        y();\n                        break; } } } } }\n",
    );
}

#[test]
fn braces_added_to_an_else_body_after_a_comment_stand_at_the_else() {
    let input = "void f(void)\n{\n\tif (a)\n\t\tx = 1;\n\telse\n\t\t// c2\n\t\tx = 2;\n}\n";
    check(
        input,
        &["--style=kr", "--add-one-line-braces"],
        "void f(void)\n{\n    if (a)\n    { x = 1; }\n    else\n        // c2\n    { x = 2; }\n}\n",
    );
}

#[test]
fn an_else_after_an_else_block_takes_the_body_level_of_its_if() {
    let input = "void f(void)\n{\n    if (o)\n        if (l) {\n            s = 1;\n        } else {\n            s = 2;\n        } else\n        s = 3;\n    x();\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_comment_after_a_dangling_else_stands_in_its_body() {
    let input = "void f(void)\n{\n    if (o)\n        if (l)\n            /*\n             */\n        {\n            s = 1;\n        } else\n            /*\n             */\n        {\n            s = 2;\n        } else\n        /*\n         * c\n         */\n    {\n        s = 3;\n    }\n}\n";
    for style in ["--style=1tbs", "--style=kr", "--style=linux"] {
        check(input, &[style], input);
    }
}

#[test]
fn an_assignment_run_in_after_its_header_continues_a_level_in() {
    let input = "void f(void) {\n    if (k) *k = (long long) aaaa() -\n                    b;\n    x = aaaa() -\n        b;\n}\n";
    check(input, &["--style=google"], input);
}

#[test]
fn a_ternary_arm_past_the_continuation_limit_stands_at_the_assigned_value() {
    let input = "void f(void) {\n    {\n        {\n            uint32_t entryFlags = ENTRY_TAKE_VALUE | ((expireAt != 0) ? ENTRY_HAS_EXPIRY :\n                                  0);\n        }\n    }\n}\n";
    check(input, &["--style=google"], input);
}

#[test]
fn a_comment_row_before_a_block_continues_no_operator_for_its_closer() {
    let input = "void f(void)\n{   x();\n    /* c\n     * d */\n    {   static const char *d[] =\n        {   /* B */\n            \"9\",\n            \"8\",\n        };\n        y();\n    }\n}\n";
    check(input, &["--style=horstmann"], input);
}

#[test]
fn braces_in_a_split_else_inside_case_blocks_stand_at_their_headers() {
    for input in [
        "void f(void)\n{   switch( c )\n    {   case 1:\n        {   if( a )\n            {   token = 1; }\n            else\n#ifndef X\n                if( b )\n                {   token = 2; }\n                else\n#endif\n                {   token = 3; }\n            break; } } }\n",
        "void f(void)\n{   switch( *zSql )\n    {   case 1:\n        {\n#ifdef X\n            unsigned char c;\n#endif\n            if( q )\n            {   switch( *zSql )\n                {   case 2:\n                    {   if( a )\n                        {   token = 1; }\n                        else\n#ifndef Y\n                            if( b )\n                            {   token = 2; }\n                            else\n#endif\n                            {   token = 3; }\n                        break; } } }\n            break; } } }\n",
    ] {
        check(input, &["--style=pico"], input);
    }
}

#[test]
fn an_else_after_one_line_blocks_ignores_an_earlier_closed_block() {
    let input = "void f(void)\n{   if( a )\n    {   if( g ) l();\n        s(); }\n    else\n\n        /*\n        ** b\n        */\n        if( b )\n        {   y(); }\n        else\n\n            /*\n            ** c\n            */\n            if( c )\n            {   z(); }\n    v(); }\n";
    check(input, &["--style=pico"], input);
}

#[test]
fn brace_rows_of_a_nested_compound_literal_stand_at_its_first_row() {
    check(
        "T info = {\n    .args = (A[]){\n        {   .name = \"condition\",\n            .subargs = (A[]){\n                {   .name = \"keepref\"\n                },\n                {   .name = \"x\"\n                },\n                {0}\n            }\n        }\n    }\n};\n",
        &["--style=horstmann"],
        "T info = {\n    .args = (A[]){\n        {   .name = \"condition\",\n            .subargs = (A[]){\n                {   .name = \"keepref\"\n                },\n                {   .name = \"x\"\n                },\n                {0}\n            }\n        }\n    }\n};\n",
    );
    check(
        "T info = {\n    .args = (A[]){\n        {   .name = \"condition\",\n            .subargs = (A[]){\n                {   .name = \"keepref\" },\n                {   .name = \"x\" },\n                {0 } } } } };\n",
        &["--style=pico"],
        "T info = {\n    .args = (A[]){\n        {   .name = \"condition\",\n            .subargs = (A[]){\n                {   .name = \"keepref\" },\n                {   .name = \"x\" },\n                {0 } } } } };\n",
    );
}

#[test]
fn a_column_one_comment_after_a_case_block_stands_in_the_case_body() {
    check(
        "void f(int op)\n{\n  switch( op ){\n    case 1: {\n      a();\n      break;\n    }\n\n/* c1\n** c2 */\n#if defined(X)\n    case 2: {\n      b();\n      break;\n    }\n#endif\n  }\n}\n",
        &["--style=pico"],
        "void f(int op)\n{   switch( op )\n    {   case 1:\n        {   a();\n            break; }\n\n            /* c1\n            ** c2 */\n#if defined(X)\n        case 2:\n        {   b();\n            break; }\n#endif\n    } }\n",
    );
}

#[test]
fn a_statement_after_a_comment_in_a_case_block_stands_in_the_block() {
    check(
        "void f(int op)\n{\n  switch( op ){\n    case 1: {\n      a();\n      break;\n    }\n    case 2: {\n      /* c1\n      ** c2\n      */\n      b(x,\n        y);\n      break;\n    }\n  }\n}\n",
        &["--style=horstmann"],
        "void f(int op)\n{   switch( op )\n    {   case 1:\n        {   a();\n            break;\n        }\n        case 2:\n        {   /* c1\n            ** c2\n            */\n            b(x,\n              y);\n            break;\n        }\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_a_directive_follows_the_comment_before_it() {
    check(
        "void f(void) {\n    switch(c) {\n    case 1: {\n        int i;\n        for(i=2; i<objc; i++) {\n            /* a */\n#ifdef T\n            /* b\n            **\n            */\n            if( x ) {\n                i++; }\n            else\n\n                /* c\n                */\n                if( y ) {\n                    z(); }\n#endif\n        }\n        break; } } }\n",
        &["--style=lisp"],
        "void f(void) {\n    switch(c) {\n    case 1: {\n        int i;\n        for(i=2; i<objc; i++) {\n            /* a */\n#ifdef T\n            /* b\n            **\n            */\n            if( x ) {\n                i++; }\n            else\n\n                /* c\n                */\n                if( y ) {\n                    z(); }\n#endif\n        }\n        break; } } }\n",
    );
}

#[test]
fn a_label_block_after_a_case_label_on_its_line_stands_in_the_case_body() {
    check(
        "void f(void) {\n  switch( op ){\n    case 1: {\n      a();\n      break;\n    }\n    case 2: lbl: {\n      int j = 1;\n      b();\n      break;\n    }\n    case 3: {\n      c();\n    }\n  }\n}\n",
        &["--style=pico"],
        "void f(void)\n{   switch( op )\n    {   case 1:\n        {   a();\n            break; }\n        case 2: lbl:\n            {   int j = 1;\n                b();\n                break; }\n        case 3:\n        {   c(); } } }\n",
    );
    check(
        "void f(void)\n{   switch( op )\n    {   case 1:\n        {   a();\n            break; }\n        case 2: lbl:\n            {   int j = 1;\n                b();\n                break; }\n        case 3:\n        {   c(); } } }\n",
        &["--style=pico"],
        "void f(void)\n{   switch( op )\n    {   case 1:\n        {   a();\n            break; }\n        case 2: lbl:\n            {   int j = 1;\n                b();\n                break; }\n        case 3:\n        {   c(); } } }\n",
    );
}

#[test]
fn padded_operators_keep_a_wider_gap_before_an_element_brace() {
    check(
        "static const struct x a[] = {\n	[A] =  { 1, \"s\" },\n	[C] =  1,\n	.d =  { 2 },\n};\nstruct y b =  { 1 };\n",
        &["--style=linux", "--pad-oper"],
        "static const struct x a[] = {\n    [A] =  { 1, \"s\" },\n    [C] =  1,\n    .d =  { 2 },\n};\nstruct y b =  { 1 };\n",
    );
}

#[test]
fn the_while_of_a_do_run_in_after_an_else_stands_at_the_else_body() {
    check(
        "void f(void)\n{\n    if (k)\n        x();\n    else do\n            *x1++ = *x++;\n        while(x < xe);\n    y();\n}\n",
        &["--style=kr"],
        "void f(void)\n{\n    if (k)\n        x();\n    else do\n            *x1++ = *x++;\n        while(x < xe);\n    y();\n}\n",
    );
}

#[test]
fn an_argument_after_a_ternary_arm_line_is_no_constructor_initializer() {
    check(
        "void f(void)\n{\n	strbuf_addf(&s->buf, \"%c%2d: \", hunk->use == USE_HUNK ? 1\n		    : hunk->use == SKIP_HUNK ? 2 : 3,\n		    (int)start_index);\n	g(a, b ? 1\n	  : 2,\n	  c);\n}\n",
        &["--style=kr"],
        "void f(void)\n{\n    strbuf_addf(&s->buf, \"%c%2d: \", hunk->use == USE_HUNK ? 1\n                : hunk->use == SKIP_HUNK ? 2 : 3,\n                (int)start_index);\n    g(a, b ? 1\n      : 2,\n      c);\n}\n",
    );
}

#[test]
fn only_extern_c_keeps_its_brace_where_the_source_put_it() {
    let input = "extern \"C\" {\nint a(void);\n}\nextern \"C++\" {\nint b(void);\n}\n";
    check(
        input,
        &["--style=allman"],
        "extern \"C\" {\n    int a(void);\n}\nextern \"C++\"\n{\n    int b(void);\n}\n",
    );
    check(
        input,
        &["--style=kr"],
        "extern \"C\" {\n    int a(void);\n}\nextern \"C++\" {\n    int b(void);\n}\n",
    );
}

#[test]
fn a_whitesmith_block_statement_in_a_case_block_follows_its_sibling() {
    check(
        "int f(void)\n{\n  switch(query) {\n    case 1: {\n      for(i = 0; i < n; i++)\n        if(g(i)) {\n          *pres1 = TRUE;\n          return 0;\n        }\n      break;\n    }\n  }\n}\n",
        &["--style=whitesmith"],
        "int f(void)\n    {\n    switch(query)\n        {\n        case 1:\n            {\n            for(i = 0; i < n; i++)\n                if(g(i))\n                    {\n                    *pres1 = TRUE;\n                    return 0;\n                    }\n            break;\n            }\n        }\n    }\n",
    );
}

#[test]
fn a_ratliff_initializer_closer_after_a_directive_split_else_stays_indented() {
    check(
        "int f(void)\n{\n  if(a) {\n    x = 1;\n  }\n  else\n#endif\n  {\n    y = 2;\n  }\n  if(b) {\n    int i;\n    struct ulbits ulflag[] = {\n      { 1, \"A\" },\n      { 0, NULL }\n    };\n    r = 1;\n  }\n}\n",
        &["--style=ratliff"],
        "int f(void) {\n    if(a) {\n        x = 1;\n        }\n    else\n#endif\n        {\n        y = 2;\n        }\n    if(b) {\n        int i;\n        struct ulbits ulflag[] = {\n                { 1, \"A\" },\n                { 0, NULL }\n            };\n        r = 1;\n        }\n    }\n",
    );
}

#[test]
fn a_continued_conditional_directive_does_not_place_a_later_type_brace() {
    check(
        "static uint64_t capsule_ntohll(uint64_t value)\n{\n#if A\n  return value;\n#elif B && \\\n  C\n  return x;\n#else\n  union {\n    uint64_t u64;\n    uint32_t u32[2];\n  } src, dst;\n\n  src.u64 = value;\n#endif\n}\n",
        &["--style=vtk"],
        "static uint64_t capsule_ntohll(uint64_t value)\n{\n#if A\n    return value;\n#elif B && \\\n  C\n    return x;\n#else\n    union\n        {\n        uint64_t u64;\n        uint32_t u32[2];\n        } src, dst;\n\n    src.u64 = value;\n#endif\n}\n",
    );
}

#[test]
fn vtk_indents_type_braces_nested_anywhere_in_a_union() {
    check(
        "struct a {\n\tunion {\n\t\tunion {\n\t\t\tint x;\n\t\t\tstruct {\n\t\t\t\tint q;\n\t\t\t} w;\n\t\t} v;\n\t} u;\n};\nunion b {\n\tstruct {\n\t\tstruct {\n\t\t\tint z;\n\t\t} k;\n\t} s;\n};\n",
        &["--style=vtk"],
        "struct a\n{\n    union\n    {\n        union\n            {\n            int x;\n            struct\n                {\n                int q;\n                } w;\n            } v;\n    } u;\n};\nunion b\n{\n    struct\n        {\n        struct\n            {\n            int z;\n            } k;\n        } s;\n};\n",
    );
}

#[test]
fn a_ratliff_kept_function_block_stands_a_level_in() {
    check(
        "int f(void)\n{ return 0; }\nvoid g(void)\n{}\n",
        &["--style=ratliff"],
        "int f(void) {\n    return 0;\n    }\nvoid g(void)\n    {}\n",
    );
}

#[test]
fn an_empty_type_body_below_its_header_takes_the_attached_brace() {
    check(
        "struct s\n{};\nunion u\n{} v;\nclass C\n{};\nnamespace n\n{}\n",
        &["--style=kr"],
        "struct s {\n};\nunion u {\n} v;\nclass C\n{};\nnamespace n\n{}\n",
    );
}

#[test]
fn an_empty_type_body_keeps_its_line_without_a_brace_style() {
    check(
        "class C\n{};\nnamespace n\n{}\n",
        &[],
        "class C\n{};\nnamespace n\n{}\n",
    );
    check(
        "class C\n{};\nnamespace n\n{}\n",
        &["--attach-classes", "--attach-namespaces"],
        "class C {\n};\nnamespace n {\n}\n",
    );
}

#[test]
fn a_comment_after_an_attached_switch_brace_stands_with_the_case_labels() {
    let input = "void f(void)\n{\n\tswitch (a)\n\t{ /* c */\n\tcase 0:\n\t\ty;\n\t}\n}\n";
    check(
        input,
        &["--style=kr", "-S"],
        "void f(void)\n{\n    switch (a) {\n        /* c */\n        case 0:\n            y;\n    }\n}\n",
    );
    check(
        input,
        &["--style=kr"],
        "void f(void)\n{\n    switch (a) {\n    /* c */\n    case 0:\n        y;\n    }\n}\n",
    );
    check(
        input,
        &["--style=ratliff"],
        "void f(void) {\n    switch (a) {\n        /* c */\n        case 0:\n            y;\n        }\n    }\n",
    );
}

#[test]
fn a_case_block_in_an_else_body_after_a_blank_line_keeps_its_levels() {
    let input = "int main(void){\n  if( a ){\n    x();\n  }else\n\n  /* c */\n  if( c ){\n    switch( op ){\n      case X: {\n        int i;\n        for(i=0; i<n; i++){\n          int v;\n          g();\n          if( v ){\n            k();\n          }\n        }\n        h();\n        break;\n      }\n    }\n  }\n  return 0;\n}\n";
    check(
        input,
        &["--style=pico"],
        "int main(void)\n{   if( a )\n    {   x(); }\n    else\n\n        /* c */\n        if( c )\n        {   switch( op )\n            {   case X:\n                {   int i;\n                    for(i=0; i<n; i++)\n                    {   int v;\n                        g();\n                        if( v )\n                        {   k(); } }\n                    h();\n                    break; } } }\n    return 0; }\n",
    );
    check(
        input,
        &["--style=horstmann"],
        "int main(void)\n{   if( a )\n    {   x();\n    }\n    else\n\n        /* c */\n        if( c )\n        {   switch( op )\n            {   case X:\n                {   int i;\n                    for(i=0; i<n; i++)\n                    {   int v;\n                        g();\n                        if( v )\n                        {   k();\n                        }\n                    }\n                    h();\n                    break;\n                }\n            }\n        }\n    return 0;\n}\n",
    );
}

#[test]
fn a_directive_before_a_case_block_brace_adds_no_level_past_the_case() {
    check(
        "void f(void)\n{\n    switch ( a )\n    {\n    case Z:\n#line 4 \"y\"\n    {\n        pop();\n    }\n    YY_BREAK\n    case W:\n        x = 1;\n        if (b)\n        {\n            c();\n        }\n        YY_BREAK\n    case V:\n        if (b)\n            c();\n    }\n}\n",
        &["--style=allman"],
        "void f(void)\n{\n    switch ( a )\n    {\n    case Z:\n#line 4 \"y\"\n        {\n            pop();\n        }\n        YY_BREAK\n    case W:\n        x = 1;\n        if (b)\n        {\n            c();\n        }\n        YY_BREAK\n    case V:\n        if (b)\n            c();\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_an_endif_leaves_a_continued_macro_body() {
    check(
        "#if A\n#define C(N) \\\n      do \\\n          x = (N); \\\n      while (0)\n#endif\n/* YYFINAL */\n",
        &["--style=kr"],
        "#if A\n#define C(N) \\\n      do \\\n          x = (N); \\\n      while (0)\n#endif\n/* YYFINAL */\n",
    );
}

#[test]
fn a_switch_closer_after_a_directive_split_else_keeps_its_column() {
    let input = "int f(void){\n  switch( x ){\n  case 1: {\n    for(i=2; i<n; i++){\n      if( a ){\n        b();\n      }else\n#endif\n      {\n        c();\n      }\n    }\n  }\n\n  } /* End */\n}\n";
    check(
        input,
        &["--style=pico"],
        "int f(void)\n{   switch( x )\n    {   case 1:\n        {   for(i=2; i<n; i++)\n            {   if( a )\n                {   b(); }\n                else\n#endif\n                {   c(); } } }\n\n    } /* End */\n}\n",
    );
    check(
        input,
        &["--style=allman"],
        "int f(void)\n{\n    switch( x )\n    {\n    case 1:\n    {\n        for(i=2; i<n; i++)\n        {\n            if( a )\n            {\n                b();\n            }\n            else\n#endif\n            {\n                c();\n            }\n        }\n    }\n\n    } /* End */\n}\n",
    );
}

#[test]
fn a_case_block_after_labels_following_a_closer_stands_at_the_first_label() {
    check(
        "void f(void){\n\tswitch (c) {\n\tcase 0: {\n\t\ta();\n\t\tbreak;\n\t} case 1: {\n\t\tb();\n\t\tbreak;\n\t} case 2: case 3: {\n\t\tc();\n\t\tbreak;\n\t}\n\t}\n}\n",
        &["--style=pico"],
        "void f(void)\n{   switch (c)\n    {   case 0:\n        {   a();\n            break; } case 1:\n        {   b();\n            break; } case 2: case 3:\n        {   c();\n            break; } } }\n",
    );
}

#[test]
fn labels_after_a_lisp_case_block_closer_keep_their_spacing() {
    check(
        "void f(void){\n\tswitch (c) {\n\tcase 0: {\n\t\ta();\n\t} case 2: case 3:\n\t\tc();\n\t}\n}\n",
        &["--style=lisp"],
        "void f(void) {\n    switch (c) {\n    case 0: {\n        a(); } case 2: case 3:\n        c(); } }\n",
    );
}

#[test]
fn a_line_comment_after_a_split_do_while_keeps_its_column() {
    let input = "void f(void) {\n  do {\n    a();\n    } while(0);                         // [for break]\n  b();\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f(void)\n{\n    do\n    {\n        a();\n    }\n    while(0);                           // [for break]\n    b();\n}\n",
    );
    check(
        input,
        &["--style=pico"],
        "void f(void)\n{   do\n    {   a(); }\n    while(0);                           // [for break]\n    b(); }\n",
    );
}

#[test]
fn a_run_in_closer_comment_keeps_its_gap_on_a_second_pass() {
    let input = "void f(void)\n{   do\n    {   if (ISZERO(rhs))                    // +/- zeros\n        {   decNumberZero(res);               // make clean\n            break; }                          // [no status to set]\n        g(); }\n    while(0); }\n";
    check(input, &["--style=pico"], input);
}

#[test]
fn a_comment_after_a_block_comment_over_lines_stands_at_its_opener() {
    check(
        "static int r(int n){\n  int yygoto;\n\n  switch( yyruleno ){\n  /* Beginning here\n  ** follows:\n  */\n/********** Begin reduce actions ****/\n%%\n/********** End reduce actions ****/\n  };\n  return 0;\n}\n",
        &["--style=ratliff"],
        "static int r(int n) {\n    int yygoto;\n\n    switch( yyruleno ) {\n            /* Beginning here\n            ** follows:\n            */\n            /********** Begin reduce actions ****/\n            %%\n            /********** End reduce actions ****/\n        };\n    return 0;\n    }\n",
    );
}

#[test]
fn block_comments_before_a_label_stand_with_the_labels_of_their_switch() {
    let input = "int f(int op){\n  switch( op ){\ncase OP_Concat: {\n  switch( op ){\n    case 1: a(); break;\n  }\n  break;\n}\n\n/* Opcode: Add\n** Synopsis\n*/\n/* Opcode: Multiply\n*/\ncase OP_Add:\ncase OP_Multiply: {\n  b();\n  break;\n}\n  }\n  return 0;\n}\n";
    check(
        input,
        &["--style=kr"],
        "int f(int op)\n{\n    switch( op ) {\n    case OP_Concat: {\n        switch( op ) {\n        case 1:\n            a();\n            break;\n        }\n        break;\n    }\n\n    /* Opcode: Add\n    ** Synopsis\n    */\n    /* Opcode: Multiply\n    */\n    case OP_Add:\n    case OP_Multiply: {\n        b();\n        break;\n    }\n    }\n    return 0;\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "int f(int op)\n    {\n    switch( op )\n        {\n        case OP_Concat:\n            {\n            switch( op )\n                {\n                case 1:\n                    a();\n                    break;\n                }\n            break;\n            }\n\n        /* Opcode: Add\n        ** Synopsis\n        */\n        /* Opcode: Multiply\n        */\n        case OP_Add:\n        case OP_Multiply:\n            {\n            b();\n            break;\n            }\n        }\n    return 0;\n    }\n",
    );
    check(
        input,
        &["--style=vtk"],
        "int f(int op)\n{\n    switch( op )\n        {\n        case OP_Concat:\n            {\n            switch( op )\n                {\n                case 1:\n                    a();\n                    break;\n                }\n            break;\n            }\n\n        /* Opcode: Add\n        ** Synopsis\n        */\n        /* Opcode: Multiply\n        */\n        case OP_Add:\n        case OP_Multiply:\n            {\n            b();\n            break;\n            }\n        }\n    return 0;\n}\n",
    );
}

#[test]
fn a_call_split_off_its_case_label_keeps_its_arguments_at_the_paren() {
    let input = "int f(void){\n  switch( prepVersion ){\n    case 1: rc = sqlite3_prepare(pDb, (const char *)pBuf,\n                                 (int)nMax, &pStmt, &zTail);\n      break;\n    case 2: rc = sqlite3_prepare_v2(pDb, (const char *)pBuf,\n                                    (int)nMax, &pStmt, &zTail);\n      break;\n  }\n  return rc;\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "int f(void)\n    {\n    switch( prepVersion )\n        {\n        case 1:\n            rc = sqlite3_prepare(pDb, (const char *)pBuf,\n                                 (int)nMax, &pStmt, &zTail);\n            break;\n        case 2:\n            rc = sqlite3_prepare_v2(pDb, (const char *)pBuf,\n                                    (int)nMax, &pStmt, &zTail);\n            break;\n        }\n    return rc;\n    }\n",
    );
    check(
        input,
        &["--style=vtk"],
        "int f(void)\n{\n    switch( prepVersion )\n        {\n        case 1:\n            rc = sqlite3_prepare(pDb, (const char *)pBuf,\n                                 (int)nMax, &pStmt, &zTail);\n            break;\n        case 2:\n            rc = sqlite3_prepare_v2(pDb, (const char *)pBuf,\n                                    (int)nMax, &pStmt, &zTail);\n            break;\n        }\n    return rc;\n}\n",
    );
}

#[test]
fn a_paren_closing_a_call_in_a_braced_define_stands_at_its_paren() {
    check(
        "#define E(id) \\\n{              \\\n  Z(           \\\n    a, b       \\\n  )            \\\n}\n",
        &["--style=kr", "--indent-preproc-define"],
        "#define E(id) \\\n    {              \\\n        Z(           \\\n                     a, b       \\\n         )            \\\n    }\n",
    );
}

#[test]
fn define_arguments_past_the_maximum_go_on_at_the_assigned_value() {
    check(
        "#define B(p) \\\n\tfor_each_hook_success = seq_try_load_hooks_long_name( \\\n\t    (p), &h[c]); \\\n\tx;\n",
        &["--style=kr", "--indent-preproc-define"],
        "#define B(p) \\\n    for_each_hook_success = seq_try_load_hooks_long_name( \\\n                            (p), &h[c]); \\\n    x;\n",
    );
}

#[test]
fn a_variable_named_template_continues_like_any_name() {
    check(
        "void f(void)\n{\n    if (opeer->host->name.len != template->host->name.len\n        || ngx_memcmp(opeer->host->name.data,\n                      template->host->name.data,\n                      template->host->name.len)\n           != 0)\n        g();\n}\n",
        &["--style=kr", "--indent-after-parens"],
        "void f(void)\n{\n    if (opeer->host->name.len != template->host->name.len\n        || ngx_memcmp(opeer->host->name.data,\n            template->host->name.data,\n            template->host->name.len)\n        != 0)\n        g();\n}\n",
    );
}

#[test]
fn a_branch_without_the_else_leaves_the_split_else_if_chain() {
    check(
        "int f(void)\n{\n  if(a)\n    return 1;\n  else if(b)\n    return 2;\n#ifdef X\n  else if(c) {\n    return 3;\n  }\n  else {\n    return 4;\n  }\n#else\n  (void)data;\n  return 0;\n#endif\n}\n",
        &["--style=google", "--break-elseifs"],
        "int f(void) {\n    if(a)\n        return 1;\n    else\n        if(b)\n            return 2;\n#ifdef X\n        else\n            if(c) {\n                return 3;\n            } else {\n                return 4;\n            }\n#else\n    (void)data;\n    return 0;\n#endif\n}\n",
    );
}

#[test]
fn an_indentable_block_opening_in_a_statement_stands_at_its_level() {
    check(
        "static bool have_dss =\n#ifdef JEMALLOC_DSS\n    true\n#else\n    false\n#endif\n    ;\n\nint f(void) {\n\treturn 0;\n}\n",
        &[
            "--style=kr",
            "--indent-preproc-cond",
            "--indent-preproc-block",
        ],
        "static bool have_dss =\n#ifdef JEMALLOC_DSS\n    true\n#else\n    false\n#endif\n    ;\n\nint f(void)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn an_initializer_run_in_after_a_block_brace_measures_from_the_brace() {
    check(
        "static void f(void)\n{\n    hooks_t hooks = {&noop_alloc_hook, &noop_dalloc_hook, &noop_expand_hook,\n        NULL};\n    g();\n}\n",
        &["--style=pico", "--indent-after-parens"],
        "static void f(void)\n{   hooks_t hooks = {&noop_alloc_hook, &noop_dalloc_hook, &noop_expand_hook,\n                    NULL };\n    g(); }\n",
    );
}

#[test]
fn a_comment_after_a_joined_case_block_closer_keeps_a_level() {
    check(
        "void f(void) {\n  switch (k) {\n    case A: {\n      g(e);\n    }  /* x */\n    case B: {\n      g(e);\n    }\n    case C: {\n      h();\n      break;\n    }\n  }\n}\n",
        &["--style=pico"],
        "void f(void)\n{   switch (k)\n    {   case A:\n            {   g(e); } /* x */\n        case B:\n        {   g(e); }\n        case C:\n        {   h();\n            break; } } }\n",
    );
}

#[test]
fn a_comment_after_a_lone_case_block_closer_keeps_a_level() {
    check(
        "void f(void) {\n  switch (k) {\n    case A: {\n      g(e);  /* c */\n    }  /* x */\n    case B: {\n      g(e);  /* c */\n    }\n    case C: {\n      g(e);  // c\n    }  /* x */\n  }\n}\n",
        &["--style=pico"],
        "void f(void)\n{   switch (k)\n    {   case A:\n        {   g(e);  /* c */\n            }  /* x */\n        case B:\n        {   g(e);  /* c */\n        }\n        case C:\n        {   g(e);  // c\n            }  /* x */\n    } }\n",
    );
}

#[test]
fn a_comment_spares_a_level_after_a_whitesmith_case_block() {
    check(
        "void f(void) {\n  switch (k) {\n    case A: {\n      g(e);\n    }\n    /* after */\n    h(); /* t */\n    case B: {\n      g(e);  /* c */\n    } // y\n    case C:\n      g(); /* z */\n  }\n}\n",
        &["--style=whitesmith"],
        "void f(void)\n    {\n    switch (k)\n        {\n        case A:\n            {\n            g(e);\n            }\n            /* after */\n            h(); /* t */\n        case B:\n            {\n            g(e);  /* c */\n            } // y\n        case C:\n            g(); /* z */\n        }\n    }\n",
    );
}

#[test]
fn a_ratliff_case_closer_keeps_its_column_before_a_comment() {
    check(
        "void f(void) {\n  switch (k) {\n    case B: {\n      g(e);\n    } // y\n    case C: {\n      g(e);\n    } /* z */\n    case D: {\n      g(e);\n    }\n  }\n}\n",
        &["--style=ratliff"],
        "void f(void) {\n    switch (k) {\n        case B: {\n            g(e);\n            } // y\n        case C: {\n            g(e);\n                } /* z */\n        case D: {\n            g(e);\n            }\n        }\n    }\n",
    );
}

#[test]
fn a_comment_after_a_whitesmith_case_block_closer_spares_a_level() {
    check(
        "void f(void) {\n  switch (k) {\n    case B: {\n      g(e);\n    } // y\n    case C: {\n      g(e);\n    } /* z */\n    case D: {\n      g(e);\n    }\n  }\n}\n",
        &["--style=whitesmith"],
        "void f(void)\n    {\n    switch (k)\n        {\n        case B:\n            {\n            g(e);\n            } // y\n        case C:\n            {\n            g(e);\n                } /* z */\n        case D:\n            {\n            g(e);\n            }\n        }\n    }\n",
    );
}

#[test]
fn an_enum_brace_in_a_case_block_stands_at_its_header() {
    check(
        "void f(void)\n{\n    switch (c)\n    {\n    case 1:\n    {\n        g();\n        enum E\n        { A, B };\n        g();\n    }\n    }\n}\n",
        &["--style=allman"],
        "void f(void)\n{\n    switch (c)\n    {\n    case 1:\n    {\n        g();\n        enum E\n        { A, B };\n        g();\n    }\n    }\n}\n",
    );
}

#[test]
fn a_run_in_enum_brace_in_a_case_block_stands_at_its_header() {
    check(
        "void f(void)\n{   switch( choice )\n    {   case 1:\n        {   g();\n            enum E\n            {   A, B };\n            g(); } } }\nenum X\n{ A, B };\nvoid h(void)\n{\n    enum Y\n    { C, D } y;\n}\n",
        &["--style=pico"],
        "void f(void)\n{   switch( choice )\n    {   case 1:\n        {   g();\n            enum E\n            {   A, B };\n            g(); } } }\nenum X\n{ A, B };\nvoid h(void)\n{   enum Y\n    { C, D } y; }\n",
    );
}

#[test]
fn a_case_brace_after_a_comment_stands_at_its_label() {
    check(
        "void f(int c)\n{\n    switch (c)\n    {\n    default:\n        /* note */\n    {\n        g();\n    }\n    }\n}\n",
        &["--style=kr"],
        "void f(int c)\n{\n    switch (c) {\n    default:\n        /* note */\n    {\n        g();\n    }\n    }\n}\n",
    );
}

#[test]
fn an_asm_statement_stays_unbroken_past_the_maximum_length() {
    check(
        "static inline int64_t xchg(int64_t* field, int64_t value)\n{\n    int64_t result = 0;\n    asm volatile (\"lock; xchgq %1, %2\" : \"=r\" (result), \"+q\" (value), \"+m\" (*field));\n    return result;\n}\n",
        &[
            "--style=google",
            "--max-code-length=80",
            "--break-after-logical",
        ],
        "static inline int64_t xchg(int64_t* field, int64_t value) {\n    int64_t result = 0;\n    asm volatile (\"lock; xchgq %1, %2\" : \"=r\" (result), \"+q\" (value), \"+m\" (*field));\n    return result;\n}\n",
    );
}

#[test]
fn rows_of_a_file_scope_preprocessor_block_stand_at_its_level() {
    check(
        "#include \"a.h\"\n#ifdef X\n/*\n * comment\n *   more\n */\nint f(int a, int b);\n/* one\n   two */\nint x = 1 +\n        2;\n#define Y 1 /* a\n             b */\nint g(void); /* t\n                c */\n#endif\n",
        &["--style=kr", "--indent-preproc-block"],
        "#include \"a.h\"\n#ifdef X\n    /*\n    * comment\n    *   more\n    */\n    int f(int a, int b);\n    /* one\n    two */\n    int x = 1 +\n    2;\n    #define Y 1 /* a\n    b */\n    int g(void); /* t\n    c */\n#endif\n",
    );
}

#[test]
fn a_closer_led_case_label_kept_on_its_line_stands_at_the_labels() {
    check(
        "void f(int c)\n{\n\tswitch (c) {\n\tcase 1: {\n\t\tg();\n\t\tbreak;\n\t} case 3: {\n\t\th();\n\t\tbreak;\n\t} default:\n\t\tbreak;\n\t}\n}\n",
        &[
            "--style=gnu",
            "--keep-one-line-statements",
            "--indent-switches",
        ],
        "void f(int c)\n{\n    switch (c)\n        {\n            case 1:\n            {\n                g();\n                break;\n            } case 3:\n            {\n                h();\n                break;\n            } default:\n                break;\n        }\n}\n",
    );
}

#[test]
fn arguments_after_a_line_leading_assignment_stand_past_its_level() {
    check(
        "void f(void)\n{\n\tif (x) {\n\t\tcommit_reflog->reflogs->short_ref\n\t\t\t= refs_shorten_unambiguous_ref(get_main_ref_store(the_repository),\n\t\t\t\t\t\t\tcommit_reflog->reflogs->ref,\n\t\t\t\t\t\t\t0);\n\t}\n}\n",
        &["--style=stroustrup", "--indent-after-parens"],
        "void f(void)\n{\n    if (x) {\n        commit_reflog->reflogs->short_ref\n            = refs_shorten_unambiguous_ref(get_main_ref_store(the_repository),\n                    commit_reflog->reflogs->ref,\n                    0);\n    }\n}\n",
    );
}

#[test]
fn unpadding_keeps_the_gap_after_a_declarators_star() {
    check(
        "typedef struct {\n    ngx_str_t             name;\n    void               *(*create_conf)(ngx_cycle_t *cycle);\n} ngx_core_module_t;\nstatic char *        (*real_fgets)(char *, int, FILE *);\nint x = a *  (b + c);\n",
        &["--style=java", "--unpad-paren", "--align-pointer=type"],
        "typedef struct {\n    ngx_str_t             name;\n    void*               (*create_conf)(ngx_cycle_t* cycle);\n} ngx_core_module_t;\nstatic char*         (*real_fgets)(char*, int, FILE*);\nint x = a * (b + c);\n",
    );
}

#[test]
fn a_block_argument_closer_indents_with_tabs() {
    check(
        "void f(void)\n{\n  M(handle, {\n    /* NOP */\n    g(1);\n  });\n}\n",
        &["--style=whitesmith", "--indent=tab=8"],
        "void f(void)\n\t{\n\tM(handle,\n\t\t{\n\t\t/* NOP */\n\t\tg(1);\n\t\t});\n\t}\n",
    );
}

#[test]
fn a_dereference_after_else_is_no_comment_row() {
    check(
        "static int parse_separator_arg(const struct option *opt, const char *arg,\n\t\t\t       int unset)\n{\n\tif (unset)\n\t\t*(const char **)opt->value = NULL;\n\telse\n\t\t*(const char **)opt->value = arg ? arg : \"\\n\";\n\treturn 0;\n}\n",
        &["--style=ratliff", "--pad-first-paren-out"],
        "static int parse_separator_arg (const struct option *opt, const char *arg,\n                                int unset) {\n    if (unset)\n        * (const char **)opt->value = NULL;\n    else\n        * (const char **)opt->value = arg ? arg : \"\\n\";\n    return 0;\n    }\n",
    );
}

#[test]
fn a_member_split_from_its_struct_type_ends_at_its_semicolon() {
    check(
        "typedef struct hnswCursor {\n    struct HNSW *index; // Reference to the index of this cursor.\n    hnswNode *current;  // Element to report when hnsw_cursor_next() is called.\n    struct hnswCursor *next; // Next cursor active.\n} hnswCursor;\n",
        &["--style=allman", "--max-code-length=60"],
        "typedef struct hnswCursor\n{\n    struct HNSW\n        *index; // Reference to the index of this cursor.\n    hnswNode *current;  // Element to report when hnsw_cursor_next() is called.\n    struct hnswCursor *next; // Next cursor active.\n} hnswCursor;\n",
    );
}

#[test]
fn a_subscript_past_the_maximum_continues_two_levels_in() {
    check(
        "LUAI_DATA const char *const luaP_opnames(NUM_OPCODES,\n1);\nLUAI_DATA const char *const luaP_opnames[NUM_OPCODES\n+1];\nvoid f(void)\n{\n    LUAI_DATA const char *const luaP_opnamesxxxxx[NUM_OPCODES\n    +1];\n    x = LUAI_DATA_const_char_const_luaP_opnamesxxxxxxx[NUM_OPCODES\n    +1];\n}\n",
        &["--style=kr"],
        "LUAI_DATA const char *const luaP_opnames(NUM_OPCODES,\n        1);\nLUAI_DATA const char *const luaP_opnames[NUM_OPCODES\n        +1];\nvoid f(void)\n{\n    LUAI_DATA const char *const luaP_opnamesxxxxx[NUM_OPCODES\n            +1];\n    x = LUAI_DATA_const_char_const_luaP_opnamesxxxxxxx[NUM_OPCODES\n            +1];\n}\n",
    );
}

#[test]
fn the_value_after_a_bare_return_declares_nothing() {
    check(
        "int f(void)\n{\n    return\n        a && b;\n}\n",
        &["--style=kr", "--align-pointer=type"],
        "int f(void)\n{\n    return\n        a && b;\n}\n",
    );
}

#[test]
fn removing_braces_keeps_a_comment_on_an_initializer_brace() {
    check(
        "static const struct spec certs[] = {\n  { /* a valid certificate */\n    \"MIIB\",\n    true\n  },\n};\n",
        &["--style=gnu", "--remove-braces"],
        "static const struct spec certs[] = {\n    { /* a valid certificate */\n        \"MIIB\",\n        true\n    },\n};\n",
    );
}

#[test]
fn a_vtk_one_line_else_block_stands_at_the_else() {
    check(
        "void f(void)\n{\n    if (a)\n        { x = 1; }\n    else\n        { y = 1; }\n}\n",
        &["--style=vtk", "--add-one-line-braces"],
        "void f(void)\n{\n    if (a)\n    { x = 1; }\n    else\n    { y = 1; }\n}\n",
    );
}

#[test]
fn a_closer_after_a_comment_only_block_closes_an_empty_block() {
    check(
        "void f(void)\n{\n    while( x ) {\n        if( a ) {\n            if( b ) {\n                g();\n            } else {\n                /* ignore */\n            }\n        } else\n            break;\n        k();\n    }\n}\n",
        &["--style=java", "--break-blocks=all"],
        "void f(void) {\n    while( x ) {\n        if( a ) {\n            if( b ) {\n                g();\n\n            } else {\n                /* ignore */\n            }\n        } else\n            break;\n\n        k();\n    }\n}\n",
    );
}

#[test]
fn indenting_after_parens_stacks_a_subscript_bracket() {
    check(
        "void f(void)\n{\n    if (x) {\n        if (y) {\n            memset(&decay->backlog[SMOOTHSTEP_NSTEPS -\n                nadvance_z], 0, (nadvance_z-1) * sizeof(size_t));\n            a = b[c +\n                d];\n            g(h[i -\n                j]);\n        }\n    }\n}\n",
        &["--style=java", "--indent-after-parens"],
        "void f(void) {\n    if (x) {\n        if (y) {\n            memset(&decay->backlog[SMOOTHSTEP_NSTEPS -\n                    nadvance_z], 0, (nadvance_z-1) * sizeof(size_t));\n            a = b[c +\n                    d];\n            g(h[i -\n                    j]);\n        }\n    }\n}\n",
    );
}

#[test]
fn a_comment_between_else_and_its_block_claims_no_braceless_body() {
    check(
        "void f(void)\n{\n    if (a)\n        b();\n    else\n        // In the middle.\n    { prev = -1; }\n\n    if (x)\n    {\n        tail = 0;\n        state = TAIL;\n    }\n}\n",
        &["--style=allman", "--keep-one-line-blocks"],
        "void f(void)\n{\n    if (a)\n        b();\n    else\n        // In the middle.\n    { prev = -1; }\n\n    if (x)\n    {\n        tail = 0;\n        state = TAIL;\n    }\n}\n",
    );
}

#[test]
fn declarators_after_a_tab_align_past_its_stop() {
    check(
        "void f(void)\n{\n    u_int\twidth_left, width_centre, width_right,\n            width_abs_centre;\n    int\tx, y,\n        z;\n}\n",
        &["--style=kr"],
        "void f(void)\n{\n    u_int\twidth_left, width_centre, width_right,\n            width_abs_centre;\n    int\tx, y,\n        z;\n}\n",
    );
}

#[test]
fn run_in_braces_leave_inline_functions_broken() {
    check(
        "class E\n{\n    void finish()\n    {\n        x();\n    }\n};\n",
        &["--style=horstmann", "--attach-inlines"],
        "class E\n{   void finish()\n    {   x();\n    }\n};\n",
    );
}

#[test]
fn a_conditional_inside_a_function_head_keeps_its_parameters_declarations() {
    check(
        "static void\n#if defined(__GNUC__) && __GNUC__ >= 11\n__attribute__((access(none, 2)))\n#endif\nhash_table_del(hashTable *tbl, void *ptr)\n{\n    return;\n}\nstatic void\n__attribute__((access(none, 2)))\nhash_table_del2(hashTable *tbl, void *ptr)\n{\n    return;\n}\n",
        &["--style=kr", "--pad-oper"],
        "static void\n#if defined(__GNUC__) && __GNUC__ >= 11\n__attribute__((access(none, 2)))\n#endif\nhash_table_del(hashTable *tbl, void *ptr)\n{\n    return;\n}\nstatic void\n__attribute__((access(none, 2)))\nhash_table_del2(hashTable *tbl, void *ptr)\n{\n    return;\n}\n",
    );
}

#[test]
fn a_compound_literal_closes_its_paren_unpadded() {
    check(
        "void f(void)\n{\n    g(c, (struct timeval) {.tv_sec = 0, .tv_usec = 500000});\n    h((int[]) {1, 2});\n}\n",
        &["--style=kr", "--pad-paren-out"],
        "void f (void)\n{\n    g (c, (struct timeval) {.tv_sec = 0, .tv_usec = 500000});\n    h ( (int[]) {1, 2});\n}\n",
    );
}

#[test]
fn added_braces_skip_headers_in_a_block_inside_parens() {
    check(
        "void f(void)\n{\n    foo(a, {\n        if (x)\n            free(b);\n    });\n    if (y)\n        g();\n}\n",
        &["--style=kr", "--add-braces"],
        "void f(void)\n{\n    foo(a, {\n        if (x)\n            free(b);\n    });\n    if (y) {\n        g();\n    }\n}\n",
    );
    check(
        "void f(void)\n{\n    foo(a, {\n        if (x)\n            free(b);\n    });\n}\n",
        &["--style=kr", "--add-one-line-braces"],
        "void f(void)\n{\n    foo(a, {\n        if (x)\n            free(b);\n    });\n}\n",
    );
}

#[test]
fn a_comment_run_in_after_a_brace_keeps_its_opener_gap_when_stripping_prefixes() {
    check(
        "int f(int rc)\n{\n  /* If the db handle must hold the mutex here.\n  ** Otherwise the read is unsafe.\n  */\n  return rc;\n}\n",
        &[
            "--style=horstmann",
            "--indent=spaces=8",
            "--remove-comment-prefix",
        ],
        "int f(int rc)\n{       /* If the db handle must hold the mutex here.\n        ** Otherwise the read is unsafe.\n        */\n        return rc;\n}\n",
    );
}

#[test]
fn lisp_aligns_a_while_continuation_on_the_line_its_closer_joins() {
    check(
        "void f(void)\n{\n    do {\n        n--;\n    } while (n &&\n             base[n - 1] != '/');\n    do {\n        n--;\n    } while (\n        n > 0\n    );\n}\n",
        &["--style=lisp", "--attach-closing-while"],
        "void f(void) {\n    do {\n        n--; } while (n &&\n                  base[n - 1] != '/');\n    do {\n        n--; } while (\n        n > 0\n    ); }\n",
    );
}

#[test]
fn a_block_head_directly_in_a_switch_body_stands_at_the_case_body() {
    let input = "void f(void)\n{\n  switch( iSub ){\n    CASE(0, \"x\") {\n      int n;\n      break;\n    }\n  }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f(void)\n    {\n    switch( iSub )\n        {\n            CASE(0, \"x\")\n                {\n                int n;\n                break;\n                }\n        }\n    }\n",
    );
    check(
        input,
        &["--style=vtk"],
        "void f(void)\n{\n    switch( iSub )\n        {\n            CASE(0, \"x\")\n                {\n                int n;\n                break;\n                }\n        }\n}\n",
    );
}

#[test]
fn an_else_if_block_in_an_else_split_by_a_directive_takes_no_extra_level() {
    check(
        "void f(void)\n{\n#ifdef X\n    if (m)\n        n = 1;\n    else\n#endif\n        if (t)\n        {\n            if (r)\n                return -1;\n            else if (n)\n            {\n                g();\n            }\n        }\n}\n",
        &["--style=allman"],
        "void f(void)\n{\n#ifdef X\n    if (m)\n        n = 1;\n    else\n#endif\n        if (t)\n        {\n            if (r)\n                return -1;\n            else if (n)\n            {\n                g();\n            }\n        }\n}\n",
    );
}

#[test]
fn rows_of_a_comment_after_a_brace_indent_with_tabs_to_the_block_body() {
    check(
        "void f(void)\n{\n\tif (a) {\n\t\t{ /* Post\n\t\t\t   $(dir) */\n\t\t}\n\t}\n    { /* q\n         r */\n    }\n}\n",
        &["--style=linux", "--indent=tab=8"],
        "void f(void)\n{\n\tif (a) {\n\t\t{ /* Post\n\t\t\t   $(dir) */\n\t\t}\n\t}\n\t{ /* q\n\t\t   r */\n\t}\n}\n",
    );
}

#[test]
fn define_parameters_indent_after_parens() {
    check(
        "#define G(type, short_type,\t\t\t\\\n    lg_size)\t\t\t\t\t\t\\\ntypedef struct {\t\t\t\t\t\t\t\\\n    type repr;\t\t\t\t\t\t\t\\\n} atomic_##short_type##_t;\n",
        &[
            "--style=java",
            "--indent-preproc-define",
            "--indent-after-parens",
        ],
        "#define G(type, short_type,\t\t\t\\\n    lg_size)\t\t\t\t\t\t\\\ntypedef struct {\t\t\t\t\t\t\t\\\n    type repr;\t\t\t\t\t\t\t\\\n} atomic_##short_type##_t;\n",
    );
}

#[test]
fn padding_outside_parens_pads_an_empty_list_after_a_close_paren() {
    check(
        "void g(void)\n{\n\t(*g)();\n\tx = (a)();\n}\n",
        &["--style=kr", "--pad-paren-out", "--unpad-paren"],
        "void g (void)\n{\n    (*g) ();\n    x = (a) ();\n}\n",
    );
}

#[test]
fn whitesmith_tabs_an_initializer_closer_aligned_past_code_as_continuation() {
    check(
        "void f(void)\n{\n\thooks_t hooks = {&a, &b,\n\t\t&c, (void *)123};\n}\nstatic const char *const k[][2] = { { \"11\", \"one\" },\n\t{ \"22\", \"two\" }\n};\n",
        &["--style=whitesmith", "--indent=tab=4"],
        "void f(void)\n\t{\n\thooks_t hooks = {&a, &b,\n\t                 &c, (void *)123\n\t                };\n\t}\nstatic const char *const k[][2] = { { \"11\", \"one\" },\n\t\t{ \"22\", \"two\" }\n\t};\n",
    );
}

#[test]
fn tabs_lead_block_comments_case_label_rows_and_close_paren_rows_as_astyle() {
    check(
        "void f(void)\n{\n\tP(h, {\n\t\t/* NOP */\n\t});\n\tTRY(L, &lj,\n\t    (*f)(L, ud);\n\t   );\n\tswitch (m) {\n\tcase A|\n\t\t\tB:\n\t\treturn 1;\n\t}\n}\n",
        &["--style=kr", "--indent=tab"],
        "void f(void)\n{\n\tP(h, {\n\t\t/* NOP */\n\t});\n\tTRY(L, &lj,\n\t    (*f)(L, ud);\n\t   );\n\tswitch (m) {\n\tcase A|\n\t\t\tB:\n\t\treturn 1;\n\t}\n}\n",
    );
}

#[test]
fn break_blocks_reads_a_bare_block_after_a_header_block_as_that_header() {
    check(
        "void g(void)\n{\n    if(rc) {\n    }\n    {\n        x();\n    }\n    rc = n(a, b);\n    {\n        y();\n    }\n    z();\n}\n",
        &["--style=kr", "--break-blocks", "--delete-empty-lines"],
        "void g(void)\n{\n    if(rc) {\n    }\n\n    {\n        x();\n    }\n\n    rc = n(a, b);\n    {\n        y();\n    }\n    z();\n}\n",
    );
}

#[test]
fn a_comment_after_an_open_paren_leaves_the_trailing_comment_its_gap() {
    check(
        "int f(void)\n{\n    if(/*c*/ n) return 1;  /* x */\n    f(/*c*/ n);  /* x */\n}\n",
        &["--style=kr"],
        "int f(void)\n{\n    if(/*c*/ n) return 1;  /* x */\n    f(/*c*/ n);  /* x */\n}\n",
    );
}

#[test]
fn parens_in_a_comment_row_open_no_parameter_list() {
    check(
        "void f(void)\n{\n    /*\n     * x (i.e. y\n     */\n    if (a & b)\n        h(a * b);\n}\n",
        &["--style=kr", "--align-pointer=type"],
        "void f(void)\n{\n    /*\n     * x (i.e. y\n     */\n    if (a & b)\n        h(a * b);\n}\n",
    );
}

#[test]
fn parens_in_unstarred_comment_rows_open_no_parameter_list() {
    check(
        "void f(void)\n{\n    /* Compare the build number separately (eg 1.9\n       is less). */\n    if(matched && buildVersion &&\n            x)\n        g();\n}\n",
        &["--style=kr", "--align-pointer=type"],
        "void f(void)\n{\n    /* Compare the build number separately (eg 1.9\n       is less). */\n    if(matched && buildVersion &&\n            x)\n        g();\n}\n",
    );
}

#[test]
fn a_call_after_a_one_line_header_takes_no_declarators() {
    check(
        "void f(int flags)\n{\n    if (color) put_str(color, F, S, flags & JV);\n    if (c) g(a * b);\n}\n",
        &["--style=kr", "--align-pointer=type"],
        "void f(int flags)\n{\n    if (color) put_str(color, F, S, flags & JV);\n    if (c) g(a * b);\n}\n",
    );
}

#[test]
fn a_block_reads_stars_between_names_in_parens_and_after_closers_as_written() {
    check(
        "void f(void)\n{\n    struct s {\n        int a;\n    } *entries;\n    STACK_OF(X509_INFO) *inf;\n    extern int g(sqlite3 *db, const char*);\n    nByte = sizeof(A)*n\n            + sizeof(B)*n;\n}\nextern int h(sqlite3 *db);\n",
        &["--style=kr", "--align-pointer=type"],
        "void f(void)\n{\n    struct s {\n        int a;\n    } *entries;\n    STACK_OF(X509_INFO) *inf;\n    extern int g(sqlite3 *db, const char*);\n    nByte = sizeof(A)*n\n            + sizeof(B)*n;\n}\nextern int h(sqlite3* db);\n",
    );
}

#[test]
fn broken_else_ifs_keep_their_level_after_a_ternary_colon_and_on_macro_rows() {
    check(
        "void f(void)\n{\n    if( a ) {\n        SWAP(a[0],a[i])\n    } else if( b ) {\n        SWAP(a[i],a[j])\n        s ? g(d) :\n        h(d);\n    }\n    x();\n}\n",
        &["--style=kr", "--break-elseifs"],
        "void f(void)\n{\n    if( a ) {\n        SWAP(a[0],a[i])\n    } else\n        if( b ) {\n            SWAP(a[i],a[j])\n            s ? g(d) :\n            h(d);\n        }\n    x();\n}\n",
    );
}

#[test]
fn removing_braces_keeps_a_bare_block_run_in_comment() {
    check(
        "void f(void)\n{\n    x();\n\n    { /* Server */\n        int a;\n        g(a);\n    }\n}\n",
        &["--style=kr", "--remove-braces"],
        "void f(void)\n{\n    x();\n\n    { /* Server */\n        int a;\n        g(a);\n    }\n}\n",
    );
}

#[test]
fn comment_rows_trim_as_astyle_trims_them_past_tabs() {
    check(
        "void f(void)\n{\n  if (size == 0)\n    {\n      /* 12 is sizeof.\n\t Use the values,\n\t the extra bytes\n\t allocated.  */\n      int extra = 1;\n    }\n}\n",
        &["--style=allman"],
        "void f(void)\n{\n    if (size == 0)\n    {\n        /* 12 is sizeof.\n        Use the values,\n         the extra bytes\n         allocated.  */\n        int extra = 1;\n    }\n}\n",
    );
}

#[test]
fn braceless_chain_in_an_else_body_split_off_by_a_directive_keeps_its_block_level() {
    let source = "void f(void)\n{\n#ifdef _LIBC\n    if (a)\n    {\n        w();\n    }\n    else\n#endif\n        for (; p >= end; --p)\n            if (q)\n            {\n                mbstate_t cur_state;\n                wchar_t wc2;\n            }\n}\n";
    check(source, &["--style=allman"], source);
}

#[test]
fn comment_after_a_broken_one_line_block_follows_the_gap_after_its_first_statement() {
    check(
        "void f(void) {\n  if (g==0) {t=*up;g+=D;}  // a\n  if (g==0) {t=*up;   g+=D;}  // b\n  if (g==0) {t=*up;}  // c\n}\n",
        &["--style=kr"],
        "void f(void)\n{\n    if (g==0) {\n        t=*up;    // a\n        g+=D;\n    }\n    if (g==0) {\n        t=*up;      // b\n        g+=D;\n    }\n    if (g==0) {\n        t=*up;   // c\n    }\n}\n",
    );
}

#[test]
fn comment_after_a_while_broken_off_a_one_line_do_body_keeps_its_joined_column() {
    check(
        "void f(void) {\n  do { rn.c = l_getc(rn.f); } while (isspace(rn.c));  /* skip spaces */\n  x();\n}\n",
        &["--style=gnu"],
        "void f(void)\n{\n    do\n        {\n            rn.c = l_getc(rn.f);\n        }\n    while (isspace(rn.c));    /* skip spaces */\n    x();\n}\n",
    );
}

#[test]
fn do_body_of_an_else_split_off_by_a_directive_keeps_its_block_level() {
    let source = "static int f(int h)\n{\n    int rc;\n#ifdef A\n    if( sz )\n    {\n        rc = 0;\n    }\n    else\n#endif\n        do\n        {\n            rc = g(h);\n        }\n        while( rc<0 );\n    return rc;\n}\n";
    check(source, &["--style=allman"], source);
}

#[test]
fn empty_block_of_a_header_nested_in_a_braceless_if_ends_its_body() {
    check(
        "static void f(int nIn){\n  if( nIn<0 ) for(nIn=0; zIn[nIn]; nIn++){}\n  while( iWidth>nIn ){ a = 1; b--; }\n  x();\n}\n",
        &["--style=allman"],
        "static void f(int nIn)\n{\n    if( nIn<0 ) for(nIn=0; zIn[nIn]; nIn++) {}\n    while( iWidth>nIn )\n    {\n        a = 1;\n        b--;\n    }\n    x();\n}\n",
    );
}

#[test]
fn brace_attached_into_a_comment_gap_takes_its_second_column() {
    check(
        "void f(int c)\n{\n    switch (c) {\n        case 2:\t/* a */\n        {\n            x();\n        }\n        case 1:      /* c */\n        {\n            y();\n        }\n        case 3:\t   /* d */\n        {\n            y();\n        }\n    }\n    if (a)      /* f */\n    {\n        z();\n    }\n}\n",
        &["--style=kr"],
        "void f(int c)\n{\n    switch (c) {\n    case 2: {\t/* a */\n        x();\n    }\n    case 1: {    /* c */\n        y();\n    }\n    case 3: {   /* d */\n        y();\n    }\n    }\n    if (a) {    /* f */\n        z();\n    }\n}\n",
    );
}

#[test]
fn statement_after_endif_skips_a_braceless_body_before_the_directive() {
    let source = "static int f(void)\n{\n#if defined(A)\n    if (b) {\n        return 1;\n    }\n#elif defined(W)\n    if (G(&m))\n        return m;\n#endif\n    return 0;\n}\n";
    check(source, &[], source);
}

#[test]
fn unstyled_brace_on_its_label_line_keeps_its_source_gap() {
    check(
        "void f(int c)\n{\n\tswitch (c) {\n\tcase 1:\t{\n\t\tx();\n\t}\n\tdefault:  {\n\t\tx();\n\t}\n\t}\n}\n",
        &[],
        "void f(int c)\n{\n    switch (c) {\n    case 1:\t{\n        x();\n    }\n    default:  {\n        x();\n    }\n    }\n}\n",
    );
}

#[test]
fn space_joining_a_closing_brace_to_its_header_moves_no_comment() {
    check(
        "static void f(D *p){\n  if( !p->rawS ){\n    p->rawS = 0;\n  }else if( p->s>=-2        /* a */\n         && p->s<=2   /* b */\n  ){\n    x();\n  }else if( b )      /* c */\n  {\n    y();\n  }else      /* d */\n  {\n    z();\n  }\n}\n",
        &[],
        "static void f(D *p) {\n    if( !p->rawS ) {\n        p->rawS = 0;\n    } else if( p->s>=-2        /* a */\n               && p->s<=2   /* b */\n             ) {\n        x();\n    } else if( b )      /* c */\n    {\n        y();\n    } else      /* d */\n    {\n        z();\n    }\n}\n",
    );
    check(
        "static void f(D *p){\n  if( !p->rawS ){\n    p->rawS = 0;\n  }else if( p->s>=-2        // a\n         && p->s<=2   // b\n  ){\n    x();\n  }else if( b )      // c\n  {\n    y();\n  }else      // d\n  {\n    z();\n  }\n}\n",
        &["--style=allman"],
        "static void f(D *p)\n{\n    if( !p->rawS )\n    {\n        p->rawS = 0;\n    }\n    else if( p->s>=-2         // a\n             && p->s<=2   // b\n           )\n    {\n        x();\n    }\n    else if( b )       // c\n    {\n        y();\n    }\n    else       // d\n    {\n        z();\n    }\n}\n",
    );
}

#[test]
fn continued_lines_of_comment_like_directives_start_at_column_one() {
    check(
        "#if A\n# error \"Must not \\\n         and SQL\"\n#endif\n#error plain \\\n   more \\\n     words\nvoid f(void)\n{\n#warning \"in \\\n    func\"\n    x();\n}\n",
        &[],
        "#if A\n# error \"Must not \\\nand SQL\"\n#endif\n#error plain \\\nmore \\\nwords\nvoid f(void)\n{\n#warning \"in \\\nfunc\"\n    x();\n}\n",
    );
}

#[test]
fn rows_after_a_string_continued_over_lines_align_to_its_paren() {
    check(
        "void f(void){\n      __asm__ __volatile__ (\"x\\n\\\n  y\"\n          : \"=r\" (r));\n      h (\"a\\\n  b\", x,\n  y);\n}\n",
        &[],
        "void f(void) {\n    __asm__ __volatile__ (\"x\\n\\\n  y\"\n                          : \"=r\" (r));\n    h (\"a\\\n  b\", x,\n       y);\n}\n",
    );
}

#[test]
fn declarators_continued_after_a_run_in_brace_stand_a_level_past_their_word() {
    check(
        "int f(void)\n{\n    uint32_t commit_count = 0,\n        tag_count = 0;\n    if (a)\n    {\n        int *p = 0,\n            *q = 1;\n    }\n}\n",
        &["--style=horstmann"],
        "int f(void)\n{   uint32_t commit_count = 0,\n                 tag_count = 0;\n    if (a)\n    {   int *p = 0,\n                 *q = 1;\n    }\n}\n",
    );
}

#[test]
fn members_continued_after_a_run_in_brace_register_nothing() {
    check(
        "static struct p\n{\n    unsigned int want_add:1,\n             want_delete:1;\n    char *r;\n};\n",
        &["--style=horstmann"],
        "static struct p\n{   unsigned int want_add:1,\n    want_delete:1;\n    char *r;\n};\n",
    );
}

#[test]
fn braces_added_around_a_do_body_on_its_own_line_break_it() {
    check(
        "void f(void)\n{\n    do\n        x++;\n    while (x);\n}\n",
        &["--style=1tbs", "--keep-one-line-blocks"],
        "void f(void)\n{\n    do {\n        x++;\n    } while (x);\n}\n",
    );
}

#[test]
fn run_in_declarators_register_the_name_or_the_array_value() {
    check(
        "int f(void)\n{\n    static const char xl[] = \"ab\",\n        xu[] = \"AB\";\n    if (x)\n    {\n        int a,\n            b;\n    }\n}\n",
        &["--style=horstmann"],
        "int f(void)\n{   static const char xl[] = \"ab\",\n                             xu[] = \"AB\";\n    if (x)\n    {   int a,\n        b;\n    }\n}\n",
    );
}

#[test]
fn comment_after_a_one_line_header_block_broken_by_break_one_line_headers_moves_in() {
    check(
        "void f(void)\n{\n  if (*s == 1) { s++; neg = 1; }  /* handle sign */\n  else if (*s == 2) s++;\n}\n",
        &[
            "--style=1tbs",
            "--keep-one-line-blocks",
            "--keep-one-line-statements",
            "--break-one-line-headers",
        ],
        "void f(void)\n{\n    if (*s == 1) {\n        s++;    /* handle sign */\n        neg = 1;\n    } else if (*s == 2) {\n        s++;\n    }\n}\n",
    );
}

#[test]
fn comment_led_row_after_a_kept_case_statement_stands_in_the_body() {
    check(
        "void f(int n)\n{\n  switch( nVal ){\n    case 6: nToken = 1;\n      /* no break */ deliberate_fall_through\n    case 5: iCol = 2;\n  }\n}\n",
        &["--keep-one-line-statements"],
        "void f(int n)\n{\n    switch( nVal ) {\n    case 6: nToken = 1;\n        /* no break */ deliberate_fall_through\n    case 5: iCol = 2;\n    }\n}\n",
    );
}

#[test]
fn every_row_of_a_comment_run_in_after_a_brace_indents_with_tabs() {
    check(
        "void f(void)\n{\n    { /* Validate that\n      ** expectations. We\n      ** checking the. */\n      int a = 0;\n    }\n}\n",
        &["--style=linux", "--indent=tab=8"],
        "void f(void)\n{\n\t{ /* Validate that\n\t\t** expectations. We\n\t\t** checking the. */\n\t\tint a = 0;\n\t}\n}\n",
    );
}

#[test]
fn block_comment_in_column_one_takes_the_indent_of_the_code_after_it() {
    check(
        "void f(void)\n{\n  switch( y ){\n  /* Beginning\n  ** follows:\n  */\n/********** Begin **/\n  x();\n  }\n}\n",
        &["--style=whitesmith"],
        "void f(void)\n    {\n    switch( y )\n        {\n            /* Beginning\n            ** follows:\n            */\n            /********** Begin **/\n            x();\n        }\n    }\n",
    );
    check(
        "void f(void)\n    {\n    switch( y )\n        {\n            /* Beginning\n            ** follows:\n            */\n            /********** Begin **/\n            x();\n        }\n    }\n",
        &["--style=whitesmith"],
        "void f(void)\n    {\n    switch( y )\n        {\n            /* Beginning\n            ** follows:\n            */\n            /********** Begin **/\n            x();\n        }\n    }\n",
    );
}

#[test]
fn row_of_adjacent_block_comments_after_a_header_stands_in_its_body() {
    let input = "void f()\n{\n    while (b)\n        /* c */ /* d */\n    {\n        x();\n    }\n    if (b)\n        /* c */ /* d */\n        x();\n}\n";
    check(input, &["--style=allman"], input);
}

// Attaching styles leave a brace off a row of block comments however many
// the row holds, as off a row of one.
#[test]
fn attached_braces_stay_off_a_row_of_adjacent_block_comments_after_a_header() {
    let input = "void f()\n{\n    while (b)\n        /* c */ /* d */\n    {\n        x();\n    }\n    if (b)\n        /* c */ /* d */\n        x();\n}\n";
    check(input, &["--style=kr"], input);
    let added = "void f()\n{\n    while (b)\n        /* c */ /* d */\n    {\n        x();\n    }\n    if (b)\n        /* c */ /* d */\n    {\n        x();\n    }\n}\n";
    check(input, &["--style=kr", "-j"], added);
    check(added, &["--style=kr", "-j"], added);
}

#[test]
fn row_of_adjacent_block_comments_in_an_indented_case_stays_in_its_body() {
    check(
        "void f(int e)\n{\n  switch (e) {\n    case 1:\n      x();\n      /* else */ /* FALLTHROUGH */\n    case 2:\n      y();\n  }\n}\n",
        &["--indent-switches"],
        "void f(int e)\n{\n    switch (e) {\n        case 1:\n            x();\n            /* else */ /* FALLTHROUGH */\n        case 2:\n            y();\n    }\n}\n",
    );
}

#[test]
fn comment_after_a_braceless_body_in_an_indented_case_stays_in_the_case() {
    check(
        "void f(int e)\n{\n  switch (e) {\n    case 1:\n      if (g(e))\n        break;\n      /* else */ /* FALLTHROUGH */\n    case 3:\n      y();\n  }\n}\n",
        &["--indent-switches"],
        "void f(int e)\n{\n    switch (e) {\n        case 1:\n            if (g(e))\n                break;\n            /* else */ /* FALLTHROUGH */\n        case 3:\n            y();\n    }\n}\n",
    );
}

#[test]
fn unnamed_pointer_or_reference_before_assignment_survives_name_alignment() {
    check(
        "const Item& =\n    Item();\n",
        &["--align-pointer=name"],
        "const Item & =\n    Item();\n",
    );
    check(
        "Item* =\n    make();\n",
        &["--align-pointer=name"],
        "Item * =\n    make();\n",
    );
}

#[test]
fn fields_of_a_returned_compound_literal_share_one_column() {
    check(
        "struct s f(void)\n{\n    return (struct s) {\n        .a = 1,\n        .b = 2,\n        .c = 3,\n    };\n}\n",
        &[],
        "struct s f(void)\n{\n    return (struct s) {\n        .a = 1,\n        .b = 2,\n        .c = 3,\n    };\n}\n",
    );
}

#[test]
fn one_line_compound_literal_assigned_through_a_member_arrow_keeps_its_closing_gap() {
    check(
        "void f(void)\n{\n    a->b = (struct s) { .c = 1 };\n    a->b += (T) { 1, 2 };\n    a->b = { 1 };\n}\n",
        &[],
        "void f(void)\n{\n    a->b = (struct s) { .c = 1 };\n    a->b += (T) { 1, 2 };\n    a->b = { 1 };\n}\n",
    );
}

#[test]
fn group_opened_at_the_end_of_a_compound_literal_row_indents_its_rows() {
    let source = "void f(void)\n{\n    y = (struct s) {\n        .addr = {\n            1, 2,\n            3, 4,\n        },\n        .c = {\n            .x = {\n                1,\n            },\n        },\n        [0] = {\n            5,\n        },\n    };\n}\n";
    check(source, &[], source);
    check(source, &["--style=kr", "--indent-switches"], source);
}

#[test]
fn initializer_rows_in_a_control_block_indent_past_a_declarator_with_a_call_in_its_bound() {
    let source = "void f(void)\n{\n    for (;;) {\n        char *argv[ARRAY_COUNT(cases[i].args) + 2] = {\n            const_cast(\"kvmemd\"),\n            NULL,\n        };\n    }\n    if (a &&\n        b(c)) {\n        x();\n    }\n}\n";
    check(
        source,
        &[
            "--style=1tbs",
            "--pad-oper",
            "--unpad-paren",
            "--pad-header",
            "--min-conditional-indent=0",
        ],
        source,
    );
}

#[test]
fn aggregate_with_run_in_first_fields_closes_at_its_statement() {
    let input = "void f(void)\n{\n    struct s sa = { .a = 1, .b = 2,\n        .c = 3\n    };\n    *p = (struct s) { .a = 1,\n        .c = 3\n    };\n    T y = (T) { .a = 1,\n        .b = 2 };\n}\n";
    let expected = "void f(void)\n{\n    struct s sa = { .a = 1, .b = 2,\n        .c = 3\n    };\n    *p = (struct s) { .a = 1,\n        .c = 3\n    };\n    T y = (T) { .a = 1,\n        .b = 2\n    };\n}\n";
    let args = [
        "--style=1tbs",
        "--pad-oper",
        "--pad-comma",
        "--unpad-paren",
        "--align-pointer=name",
        "--min-conditional-indent=0",
    ];
    check(input, &args, expected);
    check(expected, &args, expected);
}

#[test]
fn call_arguments_holding_a_multiline_compound_literal_stand_a_level_past_the_statement() {
    let input = "void m(void)\n{\n    struct udp_session *session = udp_session_create(\n        &(struct udp_session_config) {\n            .loop = loop,\n            .fd = fd,\n        },\n        other_arg);\n    int rc = udp_session_send(\n        session, &(struct msg) {\n            .len = 1,\n        });\n    session = make(\n        a, b,\n        &(struct msg) {\n            .len = 1,\n        });\n    int r = call(\n                a, b);\n}\n";
    let args = [
        "--style=1tbs",
        "--pad-oper",
        "--pad-comma",
        "--unpad-paren",
        "--align-pointer=name",
        "--min-conditional-indent=0",
    ];
    check(input, &args, input);
}

#[test]
fn compound_literal_argument_on_its_own_line_stays_with_the_arguments() {
    let source = "void m(void)\n{\n    rc = udp_send(session,\n                  &(struct msg) {\n                      .len = 1,\n                      .flags = 0,\n                  },\n                  timeout);\n    udp_send(session,\n             (struct msg) { .len = 1 },\n             timeout);\n    CHECK(run(a,\n              (struct t) {\n                  .x = 1,\n              }) == 0);\n    f(a,\n      b, (struct t) {\n          .x = 1,\n      }, c);\n    udp_send(session, &(struct msg) {\n        .len = 1,\n    });\n}\n";
    let args = [
        "--style=1tbs",
        "--pad-oper",
        "--pad-comma",
        "--unpad-paren",
        "--align-pointer=name",
        "--min-conditional-indent=0",
    ];
    check(source, &args, source);
    let allman = "void m(void)\n{\n    rc = udp_send(session,\n                  &(struct msg)\n                  {\n                      .len = 1,\n                      .flags = 0,\n                  },\n                  timeout);\n    udp_send(session,\n             (struct msg) { .len = 1 },\n             timeout);\n    CHECK(run(a,\n              (struct t)\n              {\n                  .x = 1,\n              }) == 0);\n    f(a,\n      b, (struct t)\n      {\n          .x = 1,\n      }, c);\n    udp_send(session, &(struct msg)\n    {\n        .len = 1,\n    });\n}\n";
    check(source, &["--style=allman"], allman);
    check(allman, &["--style=allman"], allman);
    let indented = "void m(void) {\n    rc = udp_send(session,\n            &(struct msg) {\n                .len = 1,\n                .flags = 0,\n            },\n            timeout);\n    udp_send(session,\n        (struct msg) { .len = 1 },\n        timeout);\n    CHECK(run(a,\n            (struct t) {\n                .x = 1,\n            }) == 0);\n    f(a,\n        b, (struct t) {\n            .x = 1,\n        }, c);\n    udp_send(session, &(struct msg) {\n        .len = 1,\n    });\n}\n";
    check(source, &["--style=java", "--indent-after-parens"], indented);
    check(
        indented,
        &["--style=java", "--indent-after-parens"],
        indented,
    );
}

#[test]
fn unclosed_paren_before_a_compound_literal_leaves_the_code_after_it_in_place() {
    // Parens the source never closes are no call's arguments, even when a
    // compound literal spanning lines follows inside them.
    let input =
        "f(a,\n\nvoid g(void)\n{\n    y();\n}\n\nstruct s v = (struct s) {\n    .a = 1,\n};\n";
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");
    let output = String::from_utf8(format_bytes(input.as_bytes(), &options).expect("format bytes"))
        .expect("utf8");
    assert!(output.contains("\n{\n    y();\n}\n"), "{output}");
    assert!(
        output.ends_with("\nstruct s v = (struct s) {\n    .a = 1,\n};\n"),
        "{output}"
    );
}

#[test]
fn lambda_trailing_return_split_onto_its_own_line_keeps_its_one_line_body() {
    // A `->` that starts its line follows the lambda's parameters above it,
    // and the lambda's one-line body stays whole, as astyle keeps it.
    let input = "void t()\n{\n    run(a,\n        [](const P &p)\n        -> std::string { return p.name; });\n}\n";
    for style in ["--style=kr", "--style=allman", "--style=gnu"] {
        check(input, &[style], input);
    }
}

#[test]
fn block_after_a_header_in_unclosed_parens_is_no_compound_literal_argument() {
    // A `for` header's parens before a brace make no cast, so the braces
    // inside parens the source leaves open hold no compound literal.
    let input = "f(a,\n\tfor (int i = NALLOCS - 1; i >= 0; i--) {\n\t}\n\t    \"Incorrect number of allocations\");\n";
    let expected = "f(a,\n  for (int i = NALLOCS - 1; i >= 0; i--)\n{\n}\n\"Incorrect number of allocations\");\n";
    check(input, &["--style=kr"], expected);
}

#[test]
fn lambda_with_a_kept_one_line_body_keeps_the_rest_of_its_line_in_breaking_styles() {
    // Splitting the lambda's header off a body that closes on the line
    // dropped the body and the code after it.
    let input = "void f()\n{\n    run([](int p) {});\n    run([](int p) { /* c */ });\n    run([](int p) { ; });\n}\n";
    for style in ["--style=allman", "--style=gnu", "--style=vtk"] {
        check(input, &[style], input);
    }
}

#[test]
fn designator_row_after_attribute_line_takes_its_initializer_level() {
    let expected = "[[nodiscard]]\nstatic inline Item make_item(Config *config)\n{\n    return (Item) {\n        .config = config,\n        .value = value_of(config),\n    };\n}\n";
    for indent in ["", "  ", "            "] {
        let input = expected.replace(
            "        .value = value_of(config),",
            &format!("{indent}.value = value_of(config),"),
        );
        for args in [
            &["--style=1tbs", "--min-conditional-indent=0"][..],
            &["--style=kr", "--min-conditional-indent=0"][..],
        ] {
            check(&input, args, expected);
        }
    }
}

#[test]
fn designator_row_after_index_initializer_in_same_function_takes_its_level() {
    let expected = "void helper(void)\n{\n    int table[] = {\n        [0] = 1,\n    };\n    result = (Item) {\n        .first = 1,\n        .second = 2,\n    };\n}\n";
    let input = expected.replace("        .second = 2,", "   .second = 2,");
    check(
        &input,
        &["--style=kr", "--min-conditional-indent=0"],
        expected,
    );
}

#[test]
fn index_group_closing_row_takes_its_initializer_level() {
    let expected = "Item items[] = {\n    [0] = {\n        .value = 1,\n    },\n};\n";
    let input = expected.replace("    },", "         },");
    for style in ["--style=kr", "--style=allman"] {
        check(&input, &[style, "--min-conditional-indent=0"], expected);
    }
}

#[test]
fn attach_return_type_leaves_a_return_type_set_off_by_a_blank_line() {
    // A blank line parts the lines, so neither is attached to the other and
    // the blank line stays.
    let input = "int value;\n\nstatic int\n\nhelper(void)\n{\n    return 0;\n}\n";
    check(input, &["--style=1tbs", "--attach-return-type"], input);
    let adjacent = "int value;\n\nstatic int\nhelper(void)\n{\n    return 0;\n}\n";
    check(
        adjacent,
        &["--style=1tbs", "--attach-return-type"],
        "int value;\n\nstatic int helper(void)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn a_c_variable_named_template_continues_its_statement_like_any_name() {
    // `template` is no keyword in C: rows continuing its assignment align as
    // for any other variable.
    let input = "void f()\n{\n    template = value +\n        result;\n    template = \"head \"\n        \"tail\";\n}\n";
    check(
        input,
        &["--mode=c"],
        "void f()\n{\n    template = value +\n               result;\n    template = \"head \"\n               \"tail\";\n}\n",
    );
}

#[test]
fn keeping_one_line_statements_keeps_statements_after_an_expanded_block() {
    // Expanding a block breaks the statements inside it; the statements after
    // its closer stay on the closer's line.
    check(
        "void f()\n{\n    if (ready) { value=0; } result=0; other=1;\n}\n",
        &["-o"],
        "void f()\n{\n    if (ready) {\n        value=0;\n    } result=0; other=1;\n}\n",
    );
    check(
        "void f()\n{\n    value=1; do { value=0; } while (ready); result=0; other=1;\n}\n",
        &["--style=kr", "-o"],
        "void f()\n{\n    value=1; do {\n        value=0;\n    } while (ready); result=0; other=1;\n}\n",
    );
}

#[test]
fn lisp_keeps_a_statement_before_a_header_whose_block_it_expands() {
    check(
        "void f()\n{\n    value=1; if (ready) { value=0; } result=0;\n}\n",
        &["--style=lisp"],
        "void f() {\n    value=1; if (ready) {\n        value=0; } result=0; }\n",
    );
    check(
        "void f()\n{\n    value=1; do { value=0; } while (ready); result=0;\n}\n",
        &["--style=lisp"],
        "void f() {\n    value=1; do {\n        value=0; }\n    while (ready); result=0; }\n",
    );
}

#[test]
fn breaking_one_line_headers_keeps_blocks_on_lines_holding_other_statements() {
    // With one-line statements and blocks kept, the block of a header that
    // follows a statement, or shares its line with more than one statement,
    // stays whole; a header alone on its line still breaks its block.
    let options = ["--break-one-line-headers", "--keep-one-line-blocks", "-o"];
    for kept in [
        "void f()\n{\n    value=1; if (ready) { value=0; }\n}\n",
        "void f()\n{\nlabel: if (ready) { value=0; }\n}\n",
    ] {
        check(kept, &options, kept);
    }
    check(
        "void f()\n{\n    if (ready) {\n        value=0; } else { result=0; }\n}\n",
        &options,
        "void f()\n{\n    if (ready) {\n        value=0;\n    } else { result=0; }\n}\n",
    );
    check(
        "void f()\n{\n    if (ready) { value=0; }\n}\n",
        &options,
        "void f()\n{\n    if (ready) {\n        value=0;\n    }\n}\n",
    );
    // A statement broken off before the header leaves it alone on its line.
    check(
        "void f()\n{\n    if (ready) {value=1; if (other) {result=0; value=0;} count++; ready=0;}\n}\n",
        &options,
        "void f()\n{\n    if (ready) {\n        value=1;\n        if (other) {\n            result=0;\n            value=0;\n        } count++;\n        ready=0;\n    }\n}\n",
    );
}

#[test]
fn without_a_brace_style_a_closing_header_stays_on_its_source_brace() {
    // The source set the header against its `}`, so it stays there whether
    // or not the block before it is expanded; a `}` the formatter adds has
    // no source place and leaves the header a line of its own unless
    // one-line statements are kept.
    let attached = "void f()\n{\n    if (ready) {\n        value=0;\n    } else {\n        result=0;\n    }\n}\n";
    for input in [
        "void f()\n{\n    if (ready) {\n        value=0;\n    } else { result=0; }\n}\n",
        "void f()\n{\n    if (ready) {\n        value=0; } else {\n        result=0;\n    }\n}\n",
        "void f()\n{\n    if (ready) { value=0; } else { result=0; }\n}\n",
    ] {
        check(input, &[], attached);
        check(input, &["--break-one-line-headers"], attached);
        check(input, &["-o"], attached);
    }
    let do_while = "void f()\n{\n    do {\n        value=0;\n    } while (ready);\n}\n";
    check(
        "void f()\n{\n    do { value=0; } while (ready);\n}\n",
        &[],
        do_while,
    );
    check(
        "void f()\n{\n    do {\n        value=0; } while (ready);\n}\n",
        &[],
        do_while,
    );
    let braceless = "void f()\n{\n    if (ready) value=0; else result=0;\n}\n";
    check(
        braceless,
        &["--add-braces"],
        "void f()\n{\n    if (ready) {\n        value=0;\n    }\n    else {\n        result=0;\n    }\n}\n",
    );
    check(braceless, &["--add-braces", "-o"], attached);
}

#[test]
fn a_comment_after_an_expanded_block_follows_the_closer_run_into_its_only_line() {
    // The comment of an expanded block moves to the block's first body line;
    // where the closer runs into that line, the comment follows the closer.
    check(
        "void f()\n{\n    if (ready) { value=0; } // done\n    if (ready) { value=0; result=1; } /* both */\n}\n",
        &["--style=lisp"],
        "void f() {\n    if (ready) {\n        value=0; } // done\n    if (ready) {\n        value=0;    /* both */\n        result=1; } }\n",
    );
}

#[test]
fn break_blocks_pads_a_header_line_kept_with_other_statements() {
    // A line a header starts takes blank lines around it with the statements
    // kept after its body; a header after a statement on its line pads
    // nothing.
    let wrap = |body: &str| format!("void f()\n{{\n    first=1;\n    {body}\n    other=1;\n}}\n");
    let padded =
        |body: &str| format!("void f()\n{{\n    first=1;\n\n    {body}\n\n    other=1;\n}}\n");
    let options = ["--style=kr", "--break-blocks", "-o"];
    for body in [
        "if (ready) value=0; result=0;",
        "if (ready) value=0; else result=0;",
    ] {
        check(&wrap(body), &options, &padded(body));
    }
    check(
        &wrap("if (ready) { value=0; } result=0;"),
        &options,
        &padded("if (ready) {\n        value=0;\n    } result=0;"),
    );
    let after_statement = wrap("x=1; if (ready) value=0;");
    check(&after_statement, &options, &after_statement);
    // A header whose blocks stay on its line pads as one on lines of its own.
    check(
        &wrap("if (ready) { value=0; } else { result=0; }"),
        &["--style=kr", "--break-blocks", "-O", "-o"],
        &padded("if (ready) { value=0; } else { result=0; }"),
    );
}

#[test]
fn max_code_length_leaves_a_kept_line_within_the_limit_whole() {
    // The limit splits only lines longer than it; a header kept after a
    // statement on a short line stays there as without the limit.
    let input = "void f()\n{\n    if (ready) value=0; else result=0;\n    value=1; if (ready) result=0;\n}\n";
    check(input, &["--style=kr", "-o"], input);
    check(input, &["--style=kr", "-o", "--max-code-length=60"], input);
}

#[test]
fn breaking_one_line_headers_keeps_a_header_after_a_kept_statement() {
    // The option parts a header from its body, not from the statement kept
    // before it, so the output formats to itself.
    let expected = "void f()\n{\n    value=1; if (ready) {\n        value=0;\n    }\n}\n";
    for input in [
        "void f()\n{\n    value=1; if (ready) { value=0; }\n}\n",
        expected,
    ] {
        check(
            input,
            &["--style=stroustrup", "-o", "--break-one-line-headers"],
            expected,
        );
    }
}

#[test]
fn breaking_one_line_headers_parts_a_header_from_the_statement_its_removed_braces_held() {
    let input = "void f()\n{\n    if (ready) { value=0; } else { result=0; }\n    while (ready) { value--; }\n}\n";
    let broken = "void f()\n{\n    if (ready)\n        value=0;\n    else\n        result=0;\n    while (ready)\n        value--;\n}\n";
    check(
        input,
        &[
            "--style=allman",
            "--keep-one-line-blocks",
            "--remove-braces",
            "--break-one-line-headers",
        ],
        broken,
    );
    // Kept one-line statements keep a line that holds several of them.
    check(
        input,
        &[
            "--keep-one-line-blocks",
            "--remove-braces",
            "--break-one-line-headers",
            "-o",
        ],
        "void f()\n{\n    if (ready)  value=0; else  result=0;\n    while (ready)\n        value--;\n}\n",
    );
    // An expanded block leaves its `else` a line of its own, as the
    // statement it held ends its line.
    check(
        input,
        &["--remove-braces", "--break-one-line-headers", "-o"],
        broken,
    );
}

#[test]
fn indented_brace_styles_indent_a_kept_bare_block_as_its_brace_wherever_it_stands() {
    // A one-line compound statement kept with its braces stands where the
    // style puts those braces, first in the body or after a statement.
    let first = "void f()\n{\n    { value=0; }\n}\n";
    let after = "void f()\n{\n    value=1;\n    { value=0; }\n}\n";
    let nested =
        "void f()\n{\n    if (ready) {\n        { value=0; }\n        result=1;\n    }\n}\n";
    for (style, first_expected, after_expected, nested_expected) in [
        (
            "--style=whitesmith",
            "void f()\n    {\n        { value=0; }\n    }\n",
            "void f()\n    {\n    value=1;\n        { value=0; }\n    }\n",
            "void f()\n    {\n    if (ready)\n        {\n            { value=0; }\n        result=1;\n        }\n    }\n",
        ),
        (
            "--style=ratliff",
            "void f() {\n        { value=0; }\n    }\n",
            "void f() {\n    value=1;\n        { value=0; }\n    }\n",
            "void f() {\n    if (ready) {\n            { value=0; }\n        result=1;\n        }\n    }\n",
        ),
        // VTK keeps a one-line block in a function's own block at its
        // statement, as after a header.
        (
            "--style=vtk",
            "void f()\n{\n    { value=0; }\n}\n",
            "void f()\n{\n    value=1;\n    { value=0; }\n}\n",
            "void f()\n{\n    if (ready)\n        {\n            { value=0; }\n        result=1;\n        }\n}\n",
        ),
    ] {
        check(first, &[style, "-O"], first_expected);
        check(after, &[style, "-O"], after_expected);
        check(nested, &[style, "-O"], nested_expected);
    }
}

#[test]
fn break_blocks_leaves_a_one_line_block_after_a_statement_unpadded() {
    // A header after a statement on its line pads nothing around a block the
    // source kept on that line, through its `else`; a block written over
    // lines still takes the blank after it.
    let wrap = |body: &str| format!("void f()\n{{\n    first=1;\n    {body}\n    other=1;\n}}\n");
    for options in [
        &["--style=kr", "--break-blocks", "-O", "-o"][..],
        &["--style=kr", "--break-blocks", "-o"][..],
    ] {
        let kept = options.contains(&"-O");
        let one = wrap("value=1; if (ready) { value=0; }");
        let one_expected = if kept {
            one.clone()
        } else {
            wrap("value=1; if (ready) {\n        value=0;\n    }")
        };
        check(&one, options, &one_expected);
        let chain = wrap("value=1; if (ready) { value=0; } else { result=0; }");
        let chain_expected = if kept {
            chain.clone()
        } else {
            wrap("value=1; if (ready) {\n        value=0;\n    } else {\n        result=0;\n    }")
        };
        check(&chain, options, &chain_expected);
    }
    check(
        &wrap("value=1; if (ready) {\n        value=0;\n    }"),
        &["--style=kr", "--break-blocks", "-o"],
        "void f()\n{\n    first=1;\n    value=1; if (ready) {\n        value=0;\n    }\n\n    other=1;\n}\n",
    );
}

#[test]
fn a_comment_after_the_closer_of_an_else_header_block_leaves_its_column() {
    let expected = "void f()\n{\n    if (ready) {\n        value = 1;\n    } else while (other) {\n            value = 0;\n        }\n    result = 1;\n    if (ready) {\n        value = 1;\n    } else switch (kind) {\n        case 1:\n            break;\n        }\n    result = 1;\n}\n";
    for comment in ["", " // done", " /* done */"] {
        let input = expected.replace("        }\n", &format!("        }}{comment}\n"));
        for style in ["--style=java", "--style=kr"] {
            check(
                &input,
                &[style],
                &input.replace(
                    "void f()\n{",
                    if style == "--style=java" {
                        "void f() {"
                    } else {
                        "void f()\n{"
                    },
                ),
            );
        }
    }
}

#[test]
fn pico_breaking_one_line_headers_keeps_a_label_with_its_header_and_block() {
    // A label leads the line of the header after it as a statement does.
    let options = ["--style=pico", "--break-one-line-headers"];
    for line in [
        "done: if (ready) { value=0; }",
        "value=1; if (ready) { value=0; }",
    ] {
        let expected = format!("void f()\n{{   {line} }}\n");
        check(
            &format!("void f()\n{{\n    {line}\n}}\n"),
            &options,
            &expected,
        );
        check(&expected, &options, &expected);
    }
}

#[test]
fn pico_breaking_one_line_headers_splits_the_statements_of_the_block_it_breaks() {
    // The block a header's line no longer holds is a block like any other:
    // its statements stand one per line, a label keeping the one after it.
    let options = ["--style=pico", "--break-one-line-headers"];
    let input = "void f()\n{\n    if (ready) { value=0; result=1; }\n    switch (kind) { case 1: value=0; break; default: break; }\n}\n";
    let expected = "void f()\n{   if (ready)\n    {   value=0;\n        result=1; }\n    switch (kind)\n    {   case 1: value=0;\n            break;\n        default: break; } }\n";
    check(input, &options, expected);
    check(expected, &options, expected);
}

#[test]
fn a_case_block_closer_a_statement_follows_stands_where_it_would_alone() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: { value=0; } break;\n    case 2:\n        result=1;\n        break;\n    }\n}\n";
    let cases: [(&[&str], &str); 7] = [
        (
            &["--style=allman", "-o"],
            "void f()\n{\n    switch (kind)\n    {\n    case 1:\n    {\n        value=0;\n    } break;\n    case 2:\n        result=1;\n        break;\n    }\n}\n",
        ),
        (
            &["--style=gnu", "-o"],
            "void f()\n{\n    switch (kind)\n        {\n        case 1:\n        {\n            value=0;\n        } break;\n        case 2:\n            result=1;\n            break;\n        }\n}\n",
        ),
        (
            &["--style=ratliff", "-o"],
            "void f() {\n    switch (kind) {\n        case 1: {\n            value=0;\n            } break;\n        case 2:\n            result=1;\n            break;\n        }\n    }\n",
        ),
        (
            &["--style=horstmann", "-o"],
            "void f()\n{   switch (kind)\n    {   case 1:\n        {   value=0;\n        } break;\n        case 2:\n            result=1;\n            break;\n    }\n}\n",
        ),
        (
            &["--style=vtk", "-o"],
            "void f()\n{\n    switch (kind)\n        {\n        case 1:\n            {\n            value=0;\n            } break;\n        case 2:\n            result=1;\n            break;\n        }\n}\n",
        ),
        (
            &["--style=allman", "-S", "-o"],
            "void f()\n{\n    switch (kind)\n    {\n        case 1:\n        {\n            value=0;\n        } break;\n        case 2:\n            result=1;\n            break;\n    }\n}\n",
        ),
        (
            &["--style=gnu", "-S", "-o"],
            "void f()\n{\n    switch (kind)\n        {\n            case 1:\n            {\n                value=0;\n            } break;\n            case 2:\n                result=1;\n                break;\n        }\n}\n",
        ),
    ];
    for (args, expected) in cases {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn whitesmith_kept_else_block_ends_the_braceless_bodies_around_it() {
    let input = "void f()\n    {\n    if (ready)\n        if (other)\n            { value=0; }\n        else\n            { result=0; }\n    switch (kind)\n        {\n        case 1:\n            value=0;\n            break;\n        }\n    x();\n    }\n";
    check(input, &["--style=whitesmith", "-O"], input);
}

#[test]
fn break_blocks_all_pads_a_closing_header_kept_on_its_brace_as_attached_styles_do() {
    // Without a brace style an `else` written after `}` stays on it; padding
    // its block then separates the `} else` line, as in attached styles.
    let input = "void f()\n{\n    if (ready) { value=0; } else { result=0; }\n    x();\n}\n";
    let expected = "void f()\n{\n    if (ready) {\n        value=0;\n\n    } else {\n        result=0;\n    }\n\n    x();\n}\n";
    check(input, &["--break-blocks=all"], expected);
    check(input, &["--style=kr", "--break-blocks=all"], expected);
}

#[test]
fn vtk_keeps_a_one_line_block_after_a_label_where_other_one_line_blocks_stand() {
    // An expanded block a label heads takes the indented brace; a kept
    // one-line block stays at its statement in a function's own block and
    // takes the brace's level within another.
    let expanded = "void f()\n{\n    done: {\n        value=0;\n    }\n}\n";
    check(
        expanded,
        &["--style=vtk"],
        "void f()\n{\ndone:\n        {\n        value=0;\n        }\n}\n",
    );
    check(
        "void f()\n{\n    done: { value=0; }\n}\n",
        &["--style=vtk", "-O"],
        "void f()\n{\ndone:\n    { value=0; }\n}\n",
    );
    check(
        "void f()\n{\n    if (x) {\n    done: { value=0; }\n    }\n}\n",
        &["--style=vtk", "-O"],
        "void f()\n{\n    if (x)\n        {\ndone:\n            { value=0; }\n        }\n}\n",
    );
}

#[test]
fn whitesmith_bare_block_after_a_labels_block_stands_at_the_statements() {
    let input = "void f()\n{\n    x();\n    again: {\n        value=0;\n    }\n    {\n        value=0;\n    }\n    y();\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    x();\nagain:\n        {\n        value=0;\n        }\n        {\n        value=0;\n        }\n    y();\n    }\n",
    );
}

#[test]
fn keeping_one_line_statements_leaves_an_else_switch_closer_at_its_column() {
    // A case brace kept on its label's line opens the case block as one
    // joined to the label later does, which leaves the switch closer alone.
    let input = "void f()\n{\n    if (ready) {\n        value=1;\n    }\n    else switch (kind) {\n        case 1: {\n            value=0;\n            break;} // one\n        default: {\n            break;}\n        } // done\n    x();\n}\n";
    let expected = "void f()\n{\n    if (ready) {\n        value=1;\n    } else switch (kind) {\n        case 1: {\n            value=0;\n            break;\n        } // one\n        default: {\n            break;\n        }\n        } // done\n    x();\n}\n";
    check(input, &["--style=kr"], expected);
    check(input, &["--style=kr", "-o"], expected);
}

#[test]
fn a_block_comment_between_a_do_closer_and_its_while_stays_between_them() {
    // Only a comment ending the line follows a broken one-line block's
    // statements; one before the `while` keeps its place.
    let input = "void f()\n{\n    do { value=0; } /* again */ while (ready);\n    x();\n}\n";
    for (style, expected) in [
        (
            "--style=kr",
            "void f()\n{\n    do {\n        value=0;\n    } /* again */ while (ready);\n    x();\n}\n",
        ),
        (
            "--style=allman",
            "void f()\n{\n    do\n    {\n        value=0;\n    } /* again */ while (ready);\n    x();\n}\n",
        ),
        (
            "--style=horstmann",
            "void f()\n{   do\n    {   value=0;\n    } /* again */ while (ready);\n    x();\n}\n",
        ),
        (
            "--style=pico",
            "void f()\n{   do { value=0; } /* again */ while (ready);\n    x(); }\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
    // A comment between a kept one-line block and its closing header holds
    // them on one line, as it does where the block's closer ran in.
    let kept = "void f()\n{\n    do { value=0; } /* again */ while (ready);\n    if (ready) { value=0; } /* c */ else { result=0; }\n}\n";
    check(kept, &["--keep-one-line-blocks"], kept);
    check(
        "void f()\n{\n    do {\n        value=0;\n    } /* again */ while (ready);\n}\n",
        &["--style=pico"],
        "void f()\n{   do\n    {   value=0; } /* again */ while (ready); }\n",
    );
    check(
        "void f()\n{   do\n    {   value=0; } /* again */ while (ready); }\n",
        &["--style=pico"],
        "void f()\n{   do\n    {   value=0; } /* again */ while (ready); }\n",
    );
}

#[test]
fn run_in_styles_attaching_closing_while_keep_it_after_a_one_line_do_block() {
    let input = "void f()\n{\n    if (ready) { value=0; } else { result=0; }\n    do { value=0; } while (ready);\n    x();\n}\n";
    check(
        input,
        &[
            "--style=lisp",
            "--attach-closing-while",
            "--keep-one-line-blocks",
        ],
        "void f() {\n    if (ready) { value=0; }\n    else { result=0; }\n    do { value=0; } while (ready);\n    x(); }\n",
    );
    check(
        input,
        &["--style=pico", "--attach-closing-while"],
        "void f()\n{   if (ready) { value=0; }\n    else { result=0; }\n    do { value=0; } while (ready);\n    x(); }\n",
    );
}

#[test]
fn break_blocks_leaves_a_kept_block_no_header_owns_unpadded() {
    // Only a header's block is padded; a bare, label or case block is not.
    let input = "void f()\n{\n    done: { value=0; }\n    value=0;\n    { value=1; }\n    value=2;\n    switch (kind) {\n    case 1: { value=0; } break;\n    case 2:\n        { value=0; }\n        break;\n    }\n    x();\n}\n";
    check(
        input,
        &["--keep-one-line-blocks", "--break-blocks"],
        "void f()\n{\ndone:\n    { value=0; }\n    value=0;\n    { value=1; }\n    value=2;\n\n    switch (kind) {\n    case 1:\n    { value=0; }\n    break;\n\n    case 2:\n    { value=0; }\n    break;\n    }\n\n    x();\n}\n",
    );
}

#[test]
fn break_blocks_pads_a_kept_else_block_on_a_line_of_its_own() {
    let input =
        "void f()\n{\n    if (ready)\n    { value=0; }\n    else\n    { result=0; }\n    x();\n}\n";
    check(
        input,
        &["--keep-one-line-blocks", "--break-blocks"],
        "void f()\n{\n    if (ready)\n    { value=0; }\n    else\n    { result=0; }\n\n    x();\n}\n",
    );
}

#[test]
fn break_blocks_reads_a_while_after_a_comment_as_its_do_blocks_end() {
    // The comment between them leaves the `while` closing the do block, so
    // no blank line opens a block before it.
    let input = "void f()\n{\n    do {\n        value=0;\n    } /* again */ while (ready);\n}\n";
    check(input, &["--break-blocks"], input);
}

#[test]
fn lisp_break_blocks_keeps_an_else_with_its_if() {
    // Breaking blocks pads around the if-else chain, not inside it.
    let input = "void f() {\n    for (;;) {\n        if (ready) break;\n        else continue; }\n    x(); }\n";
    check(
        input,
        &["--style=lisp", "--break-blocks"],
        "void f() {\n    for (;;) {\n        if (ready) break;\n        else continue; }\n\n    x(); }\n",
    );
}

#[test]
fn indented_labels_leave_a_run_in_brace_at_its_block() {
    // The label run into a block's brace takes the brace's column with it,
    // not the brace to the label's.
    let input = "void f()\n{\n    if (ready) {\n    again:\n        value=0;\n        goto again;\n    }\n}\n";
    check(
        input,
        &["--style=horstmann", "--indent-labels"],
        "void f()\n{   if (ready)\n    {   again:\n        value=0;\n        goto again;\n    }\n}\n",
    );
}

#[test]
fn ratliff_keeping_one_line_statements_leaves_a_commented_case_block_closer_at_its_body() {
    // Ratliff closes a case block at its body, a comment after the closer
    // and kept statements or not.
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n    } // done\n    break;\n    case 2:\n        result=1;\n        break;\n    }\n}\n";
    let expected = "void f() {\n    switch (kind) {\n        case 1: {\n            value=0;\n            } // done\n        break;\n        case 2:\n            result=1;\n            break;\n        }\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(input, &["--style=ratliff", "-o"], expected);
}

#[test]
fn ratliff_indented_switch_keeps_a_commented_case_block_closer_at_its_body() {
    let input = "void f()\n{\n    switch (kind) {\n        case 1: {\n            value=0;\n            break;} // one\n        default: {\n            break;}\n        } // done\n    x();\n}\n";
    check(
        input,
        &["--style=ratliff", "--indent-switches"],
        "void f() {\n    switch (kind) {\n        case 1: {\n            value=0;\n            break;\n            } // one\n        default: {\n            break;\n            }\n        } // done\n    x();\n    }\n",
    );
}

#[test]
fn ratliff_case_block_closer_kept_before_the_next_label_stands_at_its_body() {
    // Each closer a label follows on its line stands where Ratliff closes
    // the block, at its body.
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n        break;\n    } case 2: {\n        result=1;\n        break;\n    } default: done();\n    }\n}\n";
    check(
        input,
        &["--style=ratliff", "--keep-one-line-statements"],
        "void f() {\n    switch (kind) {\n        case 1: {\n            value=0;\n            break;\n            } case 2: {\n            result=1;\n            break;\n            } default: done();\n        }\n    }\n",
    );
}

#[test]
fn ratliff_indented_cases_keep_a_closer_before_the_next_label_at_the_body() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n        break;\n    } case 2: {\n        result=1;\n        break;\n    } default: done();\n    }\n}\n";
    check(
        input,
        &[
            "--style=ratliff",
            "--keep-one-line-statements",
            "--indent-cases",
        ],
        "void f() {\n    switch (kind) {\n        case 1: {\n                value=0;\n                break;\n            } case 2: {\n                result=1;\n                break;\n            } default: done();\n        }\n    }\n",
    );
}

#[test]
fn a_subscript_continued_past_the_maximum_aligns_to_its_assigned_value() {
    // As a paren does, a bracket too far out for its operand leaves the
    // continuation at the value the assignment gives, else two levels in.
    let input = "void f()\n{\n    long_type_name *variable = &container->items[index -\n            offset_value];\n    variable_name = &container->items_with_long_name[index -\n            offset_value];\n    helper(container->items_with_a_long_name_here[index -\n            offset_value]);\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    long_type_name *variable = &container->items[index -\n                               offset_value];\n    variable_name = &container->items_with_long_name[index -\n                    offset_value];\n    helper(container->items_with_a_long_name_here[index -\n            offset_value]);\n}\n",
    );
}

#[test]
fn max_code_length_continues_a_far_subscript_at_its_assigned_value() {
    let input =
        "void f()\n{\n    long_type_name *variable = &container->items[index - offset_value];\n}\n";
    check(
        input,
        &["--style=kr", "--max-code-length=60"],
        "void f()\n{\n    long_type_name *variable = &container->items[index -\n                               offset_value];\n}\n",
    );
}

#[test]
fn a_subscript_continued_after_a_comma_aligns_inside_its_bracket() {
    // A subscript holds an index, not a message's keywords.
    let input = "void f()\n{\n    value = table[LIMIT + 1,\n                  SIZE + 1];\n    table[LIMIT + 1,\n          SIZE + 1] = 0;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn indent_after_parens_continues_every_subscript_row_at_one_level() {
    let input = "void f()\n{\n    unsigned char line[LIMIT + 1 +\n                       (2 * SIZE) + 1 +\n                       (2 * MAX) + 1];\n    value = table[LIMIT + 1\n                  + SIZE + 1\n                  + MAX + 1];\n}\n";
    check(
        input,
        &["--style=kr", "--indent-after-parens"],
        "void f()\n{\n    unsigned char line[LIMIT + 1 +\n        (2 * SIZE) + 1 +\n        (2 * MAX) + 1];\n    value = table[LIMIT + 1\n            + SIZE + 1\n            + MAX + 1];\n}\n",
    );
}

#[test]
fn indent_after_parens_continues_a_subscript_after_a_comma_as_a_call() {
    // A `,` inside an open bracket ends no declarator, as one inside an
    // open paren does not.
    let input = "void f()\n{\n    value = table[LIMIT + 1,\n                  SIZE + 1];\n    value = helper(LIMIT + 1,\n                   SIZE + 1);\n}\n";
    check(
        input,
        &["--style=kr", "--indent-after-parens"],
        "void f()\n{\n    value = table[LIMIT + 1,\n            SIZE + 1];\n    value = helper(LIMIT + 1,\n            SIZE + 1);\n}\n",
    );
}

#[test]
fn a_leading_shift_aligns_inside_the_innermost_paren_left_open() {
    // A shift row continues the paren still open at its line's start, not
    // the first one its chain opened.
    let input = "void f()\n{\n    std::cout << helper(a, (b\n                            << c));\n    std::cout << helper(a) << (b\n                               << c);\n    m = 1 << (((p & (size - 1))\n               >> shift)\n              + OFFSET);\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_return_type_holding_a_paren_group_or_attribute_is_neither_broken_nor_attached() {
    // A group in parens or a `[[...]]` attribute before the name reads as
    // something other than a return type, at a head's start or on its line.
    let broken = "__attribute__((malloc)) char *duplicate(const char *text);\n[[nodiscard]] int checked(int value);\ndecltype(value) helper(int value);\nEXPORT(api) int exported(int value);\nstatic int other(int value);\n";
    check(
        broken,
        &["--style=kr", "--break-return-type-decl"],
        "__attribute__((malloc)) char *duplicate(const char *text);\n[[nodiscard]] int checked(int value);\ndecltype(value) helper(int value);\nEXPORT(api) int exported(int value);\nstatic int\nother(int value);\n",
    );
    let attached = "__attribute__((malloc)) char *\nduplicate(const char *text);\n[[nodiscard]] int\nchecked(int value);\nstatic int\nother(int value);\n";
    check(
        attached,
        &["--style=kr", "--attach-return-type-decl"],
        "__attribute__((malloc)) char *\nduplicate(const char *text);\n[[nodiscard]] int\nchecked(int value);\nstatic int other(int value);\n",
    );
}

#[test]
fn a_comment_run_into_a_lone_brace_fills_with_a_tab_under_tab_indents() {
    // Without a brace style, a comment after a lone `{` runs in at the
    // block's body column, the fill written as the indent is.
    let input = "void f()\n{\n    if (ready)\n    {   // note\n        value=0;\n    }\n    if (ready)\n    { /* note */\n        value=0;\n    }\n}\n";
    check(
        input,
        &["--indent=tab=4"],
        "void f()\n{\n\tif (ready)\n\t{\t// note\n\t\tvalue=0;\n\t}\n\tif (ready)\n\t{\t/* note */\n\t\tvalue=0;\n\t}\n}\n",
    );
    check(
        input,
        &["--indent=force-tab=4"],
        "void f()\n{\n\tif (ready)\n\t{\t// note\n\t\tvalue=0;\n\t}\n\tif (ready)\n\t{\t/* note */\n\t\tvalue=0;\n\t}\n}\n",
    );
}

#[test]
fn a_leading_shift_aligns_with_the_first_operand_inside_a_padded_paren() {
    let input = "void f()\n{\n    out << helper(first, (second\n                          << third));\n    mask = 1 << (((value & (size - 1))\n                  >> shift)\n                 + offset);\n}\n";
    check(
        input,
        &["--pad-paren-in"],
        "void f()\n{\n    out << helper( first, ( second\n                            << third ) );\n    mask = 1 << ( ( ( value & ( size - 1 ) )\n                    >> shift )\n                  + offset );\n}\n",
    );
    check(
        input,
        &["--pad-paren-out"],
        "void f()\n{\n    out << helper (first, (second\n                           << third) );\n    mask = 1 << ( ( (value & (size - 1) )\n                    >> shift)\n                  + offset);\n}\n",
    );
}

#[test]
fn an_added_do_closer_holding_its_while_stands_at_its_brace() {
    // The braces add-braces puts around a do body stand where written ones
    // would, the while attached to the closer included.
    let input = "void f()\n{\n    do\n        value=0;\n    while (ready);\n}\n";
    check(
        input,
        &["--style=gnu", "--add-braces", "--attach-closing-while"],
        "void f()\n{\n    do\n        {\n            value=0;\n        } while (ready);\n}\n",
    );
    check(
        input,
        &[
            "--style=whitesmith",
            "--add-braces",
            "--attach-closing-while",
        ],
        "void f()\n    {\n    do\n        {\n        value=0;\n        } while (ready);\n    }\n",
    );
}

#[test]
fn a_product_continuing_an_expression_keeps_its_space_before_a_paren() {
    // A `*` read as multiplication leaves the paren after it alone when
    // pointers align to their names.
    let input = "void f()\n{\n    return first +\n           second * (third - fourth);\n    value = first +\n            second * (third\n                      - fourth);\n}\n";
    check(input, &["--align-pointer=name"], input);
}

#[test]
fn a_brace_after_a_header_comment_moves_before_it_after_a_directive_too() {
    // The brace moves before the comment between it and its header
    // wherever the header stands, a directive line before it included.
    let input = "void f()\n{\n#ifdef FLAG\n    value=0;\n#endif\n    if (ready) /* why */ {\n        value=0;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n#ifdef FLAG\n    value=0;\n#endif\n    if (ready) { /* why */\n        value=0;\n    }\n}\n",
    );
}

#[test]
fn break_blocks_pads_no_directive_after_an_initializer_following_a_header_block() {
    // An initializer closes no header block, so the directives after it
    // stay where they stand, as they do with no header block before it.
    let input = "void f()\n{\n    if (ready) {\n        value=0;\n    }\n    struct Item item = {\n        .value = 1,\n    };\n#ifdef FLAG\n    value=0;\n#else\n    value=1;\n#endif\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks"],
        "void f()\n{\n    if (ready) {\n        value=0;\n    }\n\n    struct Item item = {\n        .value = 1,\n    };\n#ifdef FLAG\n    value=0;\n#else\n    value=1;\n#endif\n}\n",
    );
}

#[test]
fn pico_one_line_braces_added_before_a_comment_keep_the_unpadded_gap() {
    // Unpadding moves the comment back to its column; the added braces do
    // not, in pico as in the styles that keep the block on one line.
    let input = "void f()\n{\n    value=1;\n    if (ready) value=0;   // note\n}\n";
    check(
        input,
        &["--style=pico", "--unpad-paren", "--add-one-line-braces"],
        "void f()\n{   value=1;\n    if(ready) { value=0; }    // note\n}\n",
    );
}

#[test]
fn a_label_kept_after_a_case_block_closer_opens_its_own_block() {
    // A label kept after the closer of the case block before it opens the
    // block its line ends with, so the closer after that block stands as
    // the one before it does.
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n        break;\n    } case 2: {\n        result=1;\n        break;\n    } default: done();\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--keep-one-line-statements", "--indent-cases"],
        "void f()\n{\n    switch (kind) {\n    case 1: {\n            value=0;\n            break;\n    } case 2: {\n            result=1;\n            break;\n    } default: done();\n    }\n}\n",
    );
}

#[test]
fn whitesmith_keeps_a_statement_on_a_label_kept_after_a_case_closer() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n        break;\n    } default: done();\n    }\n}\n";
    check(
        input,
        &["--style=whitesmith", "--keep-one-line-statements"],
        "void f()\n    {\n    switch (kind)\n        {\n        case 1:\n            {\n            value=0;\n            break;\n            } default: done();\n        }\n    }\n",
    );
}

#[test]
fn a_label_kept_after_a_case_closer_stands_at_the_label_column() {
    // The line stands with the labels, a level in where braces indent with
    // their blocks.
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n        break;\n    } case 2: {\n        result=1;\n        break;\n    } default: done();\n    }\n}\n";
    check(
        input,
        &[
            "--style=allman",
            "--keep-one-line-statements",
            "--indent-cases",
        ],
        "void f()\n{\n    switch (kind)\n    {\n    case 1:\n        {\n            value=0;\n            break;\n    } case 2:\n        {\n            result=1;\n            break;\n    } default: done();\n    }\n}\n",
    );
    check(
        input,
        &["--style=vtk", "--keep-one-line-statements"],
        "void f()\n{\n    switch (kind)\n        {\n        case 1:\n            {\n            value=0;\n            break;\n            } case 2:\n            {\n            result=1;\n            break;\n            } default: done();\n        }\n}\n",
    );
    check(
        input,
        &["--style=horstmann", "--keep-one-line-statements"],
        "void f()\n{   switch (kind)\n    {   case 1:\n        {   value=0;\n            break;\n        } case 2:\n        {   result=1;\n            break;\n        } default: done();\n    }\n}\n",
    );
}

#[test]
fn removing_braces_keeps_nested_initializer_braces_together() {
    // Only statement braces go; an initializer's braces keep their line.
    let input = "void f(void)\n{\n    struct Item list[] = { {\n            .value = 1,\n            .name = \"x\",\n        }\n    };\n}\n";
    check(input, &["--style=allman", "--remove-braces"], input);
}

#[test]
fn whitesmith_closes_an_initializer_opened_after_another_on_its_line_at_its_rows() {
    // Two braces opened on one line put the inner rows two levels in, and
    // the inner closer stands with them.
    let input = "void f(void)\n{\n    struct Item list[] = { {\n            .value = 1,\n            .name = \"x\",\n        }\n    };\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f(void)\n    {\n    struct Item list[] = { {\n            .value = 1,\n            .name = \"x\",\n            }\n        };\n    }\n",
    );
    check(
        input,
        &["--style=vtk"],
        "void f(void)\n{\n    struct Item list[] = { {\n            .value = 1,\n            .name = \"x\",\n            }\n        };\n}\n",
    );
}

#[test]
fn rows_of_a_comment_run_in_after_a_brace_keep_their_spaces_past_the_block_tabs() {
    // The rows' tabs reach the block's level; the spaces after them align
    // the text past the comment's opening and stay.
    let input = "void f()\n{\n\tif (ready)\n\t{\t/* first row of a comment\n\t\t    second row\n\t\t    third row */\n\t\tvalue = 0;\n\t}\n}\n";
    check(
        input,
        &[
            "--style=horstmann",
            "--indent=tab=4",
            "--remove-comment-prefix",
        ],
        "void f()\n{\tif (ready)\n\t{\t/* first row of a comment\n\t\t    second row\n\t\t    third row */\n\t\tvalue = 0;\n\t}\n}\n",
    );
}

#[test]
fn an_added_do_closer_holding_its_while_in_a_case_block_stands_at_its_brace() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        do\n            value=0;\n        while (ready);\n    }\n    }\n}\n";
    check(
        input,
        &["--style=1tbs"],
        "void f()\n{\n    switch (kind) {\n    case 1: {\n        do {\n            value=0;\n        } while (ready);\n    }\n    }\n}\n",
    );
}

#[test]
fn nested_initializers_opened_together_close_a_level_in() {
    // After `= { {` the inner groups close a level past the row, so a
    // `}, {` row and the rows after it keep their levels.
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, {\n            .value = 2,\n        }\n    };\n}\n";
    check(input, &[], input);
    check(input, &["--style=kr"], input);
}

#[test]
fn a_closer_kept_with_its_else_past_a_comment_stands_at_the_indented_brace() {
    // Styles that indent braces part an else from its closer; the closer a
    // comment keeps on the else's line still leads it at its own column.
    let input = "void f()\n{\n    if (other) {\n        value=1;\n    } /* last */ else {\n        value=2;\n    }\n}\n";
    check(
        input,
        &["--style=gnu"],
        "void f()\n{\n    if (other)\n        {\n            value=1;\n        } /* last */ else\n        {\n            value=2;\n        }\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    if (other)\n        {\n        value=1;\n        } /* last */ else\n        {\n        value=2;\n        }\n    }\n",
    );
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    if (other) {\n        value=1;\n        } /* last */ else {\n        value=2;\n        }\n    }\n",
    );
}

#[test]
fn indent_after_parens_continues_template_parameters_a_level_in() {
    // A template's `<` continues as a paren does when parens indent.
    let input = "template <typename First,\n          typename Second>\nclass Pair {\n    First first;\n};\n";
    check(
        input,
        &["--style=kr", "--indent-after-parens"],
        "template <typename First,\n    typename Second>\nclass Pair\n{\n    First first;\n};\n",
    );
}

#[test]
fn break_blocks_all_pads_a_closing_header_a_comment_parts_from_its_brace() {
    let input = "void f()\n{\n    if (other) {\n        value=1;\n    } /* last */ else {\n        value=2;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks=all"],
        "void f()\n{\n    if (other) {\n        value=1;\n\n    } /* last */ else {\n\n        value=2;\n    }\n}\n",
    );
}

#[test]
fn lisp_nested_initializer_rows_after_a_run_in_closer_keep_their_level() {
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, {\n            .value = 2,\n        }\n    };\n}\n";
    check(
        input,
        &["--style=lisp"],
        "void f() {\n    struct Item list[] = { {\n            .value = 1, }, {\n            .value = 2, } }; }\n",
    );
}

#[test]
fn an_element_brace_after_a_closer_keeps_its_row_in_broken_brace_styles() {
    // Styles that break braces keep the next element's brace on the `}, `
    // row, which stands where the closer before it does.
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, {\n            .value = 2,\n        }\n    };\n}\n";
    check(input, &["--style=allman"], input);
    check(input, &["--style=gnu"], input);
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    struct Item list[] = { {\n            .value = 1,\n            }, {\n            .value = 2,\n            }\n        };\n    }\n",
    );
}

#[test]
fn breaking_else_ifs_keeps_unpadding_in_the_gap_before_a_comment() {
    // The `if` broken off its `else` measures its comment from the column
    // it had after the `else`, so unpadding still widens the gap.
    let input = "void f()\n{\n    if (ready) {\n        value=0;\n    } else if (other) { // second\n        value=1;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-elseifs", "--unpad-paren"],
        "void f()\n{\n    if(ready) {\n        value=0;\n    } else\n        if(other) {  // second\n            value=1;\n        }\n}\n",
    );
}

#[test]
fn pico_nested_initializer_rows_after_a_run_in_closer_keep_their_level() {
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, {\n            .value = 2,\n        }\n    };\n}\n";
    check(
        input,
        &["--style=pico"],
        "void f()\n{   struct Item list[] = { {\n            .value = 1, }, {\n            .value = 2, } }; }\n",
    );
}

#[test]
fn indent_after_parens_leaves_a_lambda_argument_opening_its_body_at_the_statement() {
    // A row inside the call that opens a lambda body stands with the body
    // and its closer, as it does when parens align.
    let input = "void f()\n{\n    helper(first,\n           [](int value) {\n               return value;\n           });\n}\n";
    check(
        input,
        &["--style=kr", "--indent-after-parens"],
        "void f()\n{\n    helper(first,\n    [](int value) {\n        return value;\n    });\n}\n",
    );
}

#[test]
fn whitesmith_keeps_a_range_for_colon_inside_its_parens() {
    let input =
        "void f()\n{\n    for (auto &item : items) {\n        total += item.value;\n    }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    for (auto &item : items)\n        {\n        total += item.value;\n        }\n    }\n",
    );
}

#[test]
fn a_block_closing_after_a_message_in_a_method_stands_at_its_header() {
    // Only the method's own closer stands flush; a block inside it closes
    // at its header after a message as after any statement.
    let input = "@implementation Item\n- (void)update\n{\n    if (value) {\n        [self reset];\n    }\n}\n@end\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_declarator_starting_with_default_after_a_closer_is_no_label() {
    let input = "extern struct Item {\n    int value;\n    char *name;\n} default_item;\n";
    check(
        input,
        &["--style=vtk"],
        "extern struct Item\n{\n    int value;\n    char *name;\n} default_item;\n",
    );
}

#[test]
fn an_objc_method_closer_after_a_message_stands_where_the_style_closes_bodies() {
    let input = "@implementation Item\n- (int)total {\n    return [[self items] count] + [self.other countWith:1\n                                              and:2];\n}\n@end\n";
    check(
        input,
        &["--style=ratliff"],
        "@implementation Item\n- (int)total {\n    return [[self items] count] + [self.other countWith:1\n                                              and:2];\n    }\n@end\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "@implementation Item\n- (int)total\n    {\n    return [[self items] count] + [self.other countWith:1\n                                              and:2];\n    }\n@end\n",
    );
}

#[test]
fn a_lambda_argument_opening_its_body_after_a_ternary_stands_at_the_statement() {
    let input = "void f()\n{\n    result = ready ? first\n             : third;\n    helper(first,\n           [](int value) {\n               return value;\n           });\n}\n";
    check(
        input,
        &["--style=linux"],
        "void f()\n{\n    result = ready ? first\n             : third;\n    helper(first,\n    [](int value) {\n        return value;\n    });\n}\n",
    );
}

#[test]
fn an_alias_pointer_declarator_follows_the_inside_paren_options() {
    let input = "using Callback = void (*)(int value);\nusing Handler = void (  &  )(int);\n";
    check(
        input,
        &["--pad-paren-in"],
        "using Callback = void ( * )( int value );\nusing Handler = void (  &  )( int );\n",
    );
    check(
        input,
        &["--pad-paren-in", "--unpad-paren"],
        "using Callback = void ( * )( int value );\nusing Handler = void ( & )( int );\n",
    );
    check(
        "using Callback = void ( * )(int value);\nusing Handler = void ( ^)(int);\n",
        &["--unpad-paren"],
        "using Callback = void (*)(int value);\nusing Handler = void (^)(int);\n",
    );
}

#[test]
fn indent_after_parens_keeps_message_keywords_aligned_wherever_the_message_starts() {
    let input = "@implementation Item\n- (void)update\n{\n    [self apply:value\n with:other];\n    result = [self apply:value\n with:other];\n}\n@end\n";
    check(
        input,
        &["--indent-after-parens"],
        "@implementation Item\n- (void)update\n{\n    [self apply:value\n          with:other];\n    result = [self apply:value\n                   with:other];\n}\n@end\n",
    );
}

#[test]
fn a_block_literal_brace_follows_the_style_at_file_scope_as_in_a_body() {
    let input = "void (^handler)(int) = ^(int value) {\n    helper(value);\n};\nvoid f()\n{\n    void (^local)(int) = ^(int value) {\n        helper(value);\n    };\n}\n";
    check(
        input,
        &["--style=allman"],
        "void (^handler)(int) = ^(int value)\n{\n    helper(value);\n};\nvoid f()\n{\n    void (^local)(int) = ^(int value)\n    {\n        helper(value);\n    };\n}\n",
    );
}

#[test]
fn an_element_brace_after_a_comma_keeps_its_row_in_broken_brace_styles() {
    // A row of values or of braces ending in an element's `{` keeps it; the
    // styles that indent braces close it at its elements.
    let input = "void f()\n{\n    Item items[] = {\n        1, {\n            7,\n            8\n        },\n        { 1, 2 }, {\n            3, 4\n        }\n    };\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    Item items[] =\n    {\n        1, {\n            7,\n            8\n        },\n        { 1, 2 }, {\n            3, 4\n        }\n    };\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    Item items[] =\n        {\n        1, {\n            7,\n            8\n            },\n            { 1, 2 }, {\n            3, 4\n            }\n        };\n    }\n",
    );
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    Item items[] = {\n        1, {\n            7,\n            8\n            },\n            { 1, 2 }, {\n            3, 4\n            }\n        };\n    }\n",
    );
}

#[test]
fn a_run_in_brace_after_a_one_line_switch_fills_as_anywhere() {
    let input = "void f()\n{\n    switch (kind) { case 1: value=0; break; default: break; }\n    struct Item item = {\n        .value = 1,\n        .name = \"x\",\n    };\n}\n";
    check(
        input,
        &["--style=pico"],
        "void f()\n{   switch (kind) { case 1: value=0; break; default: break; }\n    struct Item item =\n    {   .value = 1,\n        .name = \"x\", }; }\n",
    );
}

#[test]
fn vtk_keeps_a_case_body_after_a_label_on_its_closer_at_the_label_body() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n    } case 2:\n        value=1;\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=vtk", "-o"],
        "void f()\n{\n    switch (kind)\n        {\n        case 1:\n            {\n            value=0;\n            } case 2:\n            value=1;\n            break;\n        }\n}\n",
    );
}

#[test]
fn indented_case_blocks_leave_the_statements_after_a_closer_label_at_the_label_body() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n    } case 2:\n        value=1;\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=pico", "--indent-cases"],
        "void f()\n{   switch (kind)\n    {   case 1:\n            {   value=0; } case 2:\n            value=1;\n            break; } }\n",
    );
}

#[test]
fn an_indented_case_block_keeps_its_level_whatever_label_follows_its_closer() {
    // A label run in after the closer moves neither the block's row nor
    // its statements, whether a case or a default.
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n    } case 2:\n        value=1;\n        break;\n    case 3: {\n        value=2;\n    } default:\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=pico", "--indent-cases"],
        "void f()\n{   switch (kind)\n    {   case 1:\n            {   value=0; } case 2:\n            value=1;\n            break;\n        case 3:\n            {   value=2; } default:\n            break; } }\n",
    );
    check(
        input,
        &["--style=lisp", "--indent-cases"],
        "void f() {\n    switch (kind) {\n    case 1: {\n            value=0; } case 2:\n        value=1;\n        break;\n    case 3: {\n            value=2; } default:\n        break; } }\n",
    );
    check(
        input,
        &["--style=pico"],
        "void f()\n{   switch (kind)\n    {   case 1:\n        {   value=0; } case 2:\n            value=1;\n            break;\n        case 3:\n        {   value=2; } default:\n            break; } }\n",
    );
}

#[test]
fn ratliff_returns_a_value_row_after_an_element_brace_row_to_the_elements() {
    let input = "void f()\n{\n    int table[][2] = {\n        { 1, 2 }, {\n            3, 4\n        },\n        5, {\n            6, 7\n        }\n    };\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    int table[][2] = {\n            { 1, 2 }, {\n            3, 4\n            },\n        5, {\n            6, 7\n            }\n        };\n    }\n",
    );
}

#[test]
fn pico_aligns_a_paren_continuation_after_a_closer_run_into_the_row_before() {
    // The rows of a paren the joined closer's line leaves open move with it.
    let input = "void f()\n{\n    do {\n        value++;\n    } while (value < count &&\n             ready);\n}\n";
    check(
        input,
        &["--style=pico", "--attach-closing-while"],
        "void f()\n{   do\n    {   value++; } while (value < count &&\n                          ready); }\n",
    );
}

#[test]
fn a_condition_continued_after_an_indented_closer_aligns_inside_its_paren() {
    let input = "void f()\n{\n    do {\n        value++;\n    } while (value < count &&\n             ready);\n}\n";
    check(
        input,
        &["--style=whitesmith", "--attach-closing-while"],
        "void f()\n    {\n    do\n        {\n        value++;\n        } while (value < count &&\n                 ready);\n    }\n",
    );
}

#[test]
fn a_closer_keeping_the_next_element_brace_stands_at_its_own_brace_in_gnu_and_horstmann() {
    let input = "void f()\n{\n    struct Item refs[2] = {\n        {\n            .name = \"b\",\n        }, {\n            .name = \"a\",\n        }\n    };\n}\n";
    check(
        input,
        &["--style=gnu"],
        "void f()\n{\n    struct Item refs[2] =\n    {\n        {\n            .name = \"b\",\n        }, {\n            .name = \"a\",\n        }\n    };\n}\n",
    );
    check(
        input,
        &["--style=horstmann"],
        "void f()\n{   struct Item refs[2] =\n    {   {   .name = \"b\",\n        }, {\n            .name = \"a\",\n        }\n    };\n}\n",
    );
}

#[test]
fn break_blocks_pads_a_header_after_a_labelled_one_line_block_as_after_any_block() {
    let input = "void f()\n{\n    done: if (ready) { value=0; }\n    switch (kind) {\n    default:\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=pico", "--break-blocks"],
        "void f()\n{   done: if (ready) { value=0; }\n\n    switch (kind)\n    {   default:\n            break; } }\n",
    );
}

#[test]
fn break_blocks_keeps_a_stream_continuation_aligned_after_the_blank_it_adds() {
    let input = "void f()\n{\n    if (ready) {\n        value++;\n    }\n    std::cout << first\n              << second << std::endl;\n}\n";
    check(
        input,
        &["--break-blocks"],
        "void f()\n{\n    if (ready) {\n        value++;\n    }\n\n    std::cout << first\n              << second << std::endl;\n}\n",
    );
}

#[test]
fn indent_after_parens_indents_a_lambda_argument_after_a_ternary_statement() {
    let input = "void f()\n{\n    result = ready\n        ? value\n        : other;\n    sort(first, second,\n         [](int value)\n    {\n        return value;\n    });\n}\n";
    check(
        input,
        &["--style=allman", "--indent-after-parens"],
        "void f()\n{\n    result = ready\n        ? value\n        : other;\n    sort(first, second,\n        [](int value)\n    {\n        return value;\n    });\n}\n",
    );
}

#[test]
fn a_braceless_body_in_an_indented_macro_indents_with_tabs() {
    let input = "#define CHECK(A) \\\n  do {                         \\\n    helper(A);                 \\\n    if(result)                 \\\n      goto done;               \\\n  } while(0)\n";
    check(
        input,
        &["--indent=tab=4", "--indent-preproc-define"],
        "#define CHECK(A) \\\n\tdo {                         \\\n\t\thelper(A);                 \\\n\t\tif(result)                 \\\n\t\t\tgoto done;               \\\n\t} while(0)\n",
    );
    // A header whose condition spans rows opens its body once it closes.
    check(
        "#define LOOP(A) \\\n  if (a &&                  \\\n      b)                    \\\n    value = 1;              \\\n  else                      \\\n    value = 2\n",
        &["--indent=tab=4", "--indent-preproc-define"],
        "#define LOOP(A) \\\n\tif (a &&                  \\\n\t        b)                    \\\n\t\tvalue = 1;              \\\n\telse                      \\\n\t\tvalue = 2\n",
    );
}

#[test]
fn pad_oper_pads_a_product_after_any_sizeof_operand() {
    let input = "void f()\n{\n    memmove(pool, pool, sizeof(pool[0])*count);\n    value = sizeof(item)*count;\n}\n";
    check(
        input,
        &["--pad-oper"],
        "void f()\n{\n    memmove(pool, pool, sizeof(pool[0]) * count);\n    value = sizeof(item) * count;\n}\n",
    );
}

#[test]
fn a_split_kept_case_statement_aligns_after_the_padding_inside_its_paren() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: snprintf(buffer, length, \"Unknown system error %d\", error);\n    }\n}\n";
    check(
        input,
        &[
            "--max-code-length=60",
            "--pad-paren-in",
            "--keep-one-line-statements",
        ],
        "void f()\n{\n    switch ( kind ) {\n    case 1: snprintf( buffer, length, \"Unknown system error %d\",\n                          error );\n    }\n}\n",
    );
}

#[test]
fn an_argument_after_a_compound_literal_aligns_inside_its_call() {
    // A literal's braces leave the call's parenthesis open: every argument
    // row after one aligns inside it, as after any argument.
    let input = "void f()\n{\n    helper((struct Item){ 1, 2 },\n           other);\n    helper((struct Item){ 1, 2 }, second,\n           other);\n    helper(first, (int[]){ 1, 2 }, second,\n           other);\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    helper((struct Item) { 1, 2 },\n           other);\n    helper((struct Item) { 1, 2 }, second,\n           other);\n    helper(first, (int[]) { 1, 2 }, second,\n           other);\n}\n",
    );
}

#[test]
fn a_block_broken_off_its_header_with_an_attribute_closes_under_its_line() {
    // The attributed block becomes the header's body; its rows and closer
    // follow the line it opens on.
    let input = "void f()\n{\n    if (ready) [[likely]] {\n        value = 1;\n    }\n}\n";
    check(
        input,
        &["--break-one-line-headers"],
        "void f()\n{\n    if (ready)\n        [[likely]] {\n            value = 1;\n        }\n}\n",
    );
}

#[test]
fn a_kept_one_line_block_ends_its_assignment_before_a_trailing_comment() {
    // The assignment inside the block ended with it; the statement after
    // the commented line continues from its own columns.
    let input = "void f()\n{\n    if (ready) { value=0; }   // note\n    value = table[first\n                  + second];\n}\n";
    check(input, &["--keep-one-line-blocks"], input);
    check(
        "void f()\n{\n    if (ready) value=0;   // note\n    value = table[first\n                  + second];\n}\n",
        &["--add-one-line-braces"],
        input,
    );
}

#[test]
fn an_else_broken_off_its_closer_leaves_the_comment_where_the_joined_line_had_it() {
    // As for a `while` broken off its `do` body, the closer and the space
    // after it stay in the comment's gap; padding the else takes from it.
    let input = "void f()\n{\n    if (ready) { a = 1; } else { b = 2; } // note\n    if (ready) { a = 1; }   else{ b = 2; } // note\n}\n";
    check(
        input,
        &["--keep-one-line-blocks"],
        "void f()\n{\n    if (ready) { a = 1; }\n    else { b = 2; }   // note\n    if (ready) { a = 1; }\n    else { b = 2; }    // note\n}\n",
    );
}

#[test]
fn whitesmith_keeps_the_statements_of_a_case_block_after_a_closer_label_at_its_brace() {
    let input = "void f()\n    {\n    switch (kind)\n        {\n        case 1:\n            {\n            value = 0;\n            break;\n            } case 2:\n            {\n            size_t mib[3];\n            size_t miblen = 1;\n            helper(first,\n                   second);\n            break;\n            }\n        }\n    }\n";
    check(
        input,
        &["--style=whitesmith", "--keep-one-line-statements"],
        input,
    );
}

#[test]
fn a_lambda_inside_a_lambda_body_breaks_its_brace_as_any_lambda() {
    let input = "void f()\n{\n    auto outer = [&](int value) {\n        auto inner = [value](int other) {\n            return value + other;\n        };\n        return inner(1);\n    };\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    auto outer = [&](int value)\n    {\n        auto inner = [value](int other)\n        {\n            return value + other;\n        };\n        return inner(1);\n    };\n}\n",
    );
}

#[test]
fn a_structured_binding_reference_aligns_as_any_reference_declarator() {
    let input =
        "void f()\n{\n    for (auto &[key, value] : items) {\n        total += value;\n    }\n}\n";
    check(
        input,
        &["--align-pointer=type"],
        "void f()\n{\n    for (auto& [key, value] : items) {\n        total += value;\n    }\n}\n",
    );
}

#[test]
fn an_added_closer_run_into_its_row_keeps_the_comment_gap() {
    let input = "void f()\n{\n    if (ready) value=0;   // note\n}\n";
    check(
        input,
        &["--style=pico", "--break-one-line-headers", "--add-braces"],
        "void f()\n{   if (ready)\n    {   value=0; }   // note\n}\n",
    );
}

#[test]
fn a_definition_with_parameter_declarations_breaks_and_attaches_its_return_type() {
    // A definition declaring its parameters before its body is a function
    // definition like any other.
    let joined = "int helper(value, other)\nint value;\nchar *other;\n{\n    return value;\n}\n";
    let broken = "int\nhelper(value, other)\nint value;\nchar *other;\n{\n    return value;\n}\n";
    check(joined, &["--break-return-type"], broken);
    check(broken, &["--attach-return-type"], joined);
}

#[test]
fn break_blocks_keeps_a_label_after_a_case_closer_with_its_statements() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n    } case 2:\n        value=1;\n        break;\n    }\n}\n";
    check(input, &["--style=kr", "--break-blocks", "-o"], input);
}

#[test]
fn rows_after_a_sign_led_first_element_stand_with_it() {
    // A sign opening the list's first element is no operator continuing
    // the rows after it.
    let input = "void f()\n{\n    int list[] = {\n        -1, 2,\n        3, 4,\n    };\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn an_element_after_a_directive_breaks_its_closer_as_any_element() {
    let input = "static Item items[] = {\n\n#if (HAVE_THING)\n\n    { name(\"second\"),\n      helper,\n      NULL },\n\n#endif\n\n    { name(\"third\"),\n      helper,\n      NULL },\n\n    null_item\n};\n";
    check(
        input,
        &["--style=kr"],
        "static Item items[] = {\n\n#if (HAVE_THING)\n\n    {\n        name(\"second\"),\n        helper,\n        NULL\n    },\n\n#endif\n\n    {\n        name(\"third\"),\n        helper,\n        NULL\n    },\n\n    null_item\n};\n",
    );
}

#[test]
fn a_comment_set_against_a_brace_keeps_its_column_when_the_closer_before_breaks_off() {
    let input = "void f()\n{\n    if (ready) {\n        value = 1;\n    } else if ((p = strstr(a, b))) {//Importing\n        value = 2;\n    }\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    if (ready) {\n        value = 1;\n        }\n    else if ((p = strstr(a, b))) {  //Importing\n        value = 2;\n        }\n    }\n",
    );
}

#[test]
fn ratliff_closes_blocks_deep_in_a_commented_else_chain_at_their_bodies() {
    // Each `else` a comment parts from its `if` nests the chain a level;
    // closers deep in it still stand at their blocks' bodies.
    let input = "void f()\n{\n  if( first ){\n    file = open_file(name, mode);\n    if( file==0 ){\n      stop(1);\n    }\n    if( status!=READY ){\n      fail(status);\n    }else if( size>0 ){\n      fail(size);\n    }\n    close_file(file);\n  }else\n\n  /* second */\n  if( second ){\n    file = open_file(name, mode);\n    if( file==0 ){\n      stop(1);\n    }\n    if( status!=READY ){\n      fail(status);\n    }else if( size>0 ){\n      fail(size);\n    }\n    close_file(file);\n  }else\n\n  /* third */\n  if( third ){\n    file = open_file(name, mode);\n    if( file==0 ){\n      stop(1);\n    }\n    if( status!=READY ){\n      fail(status);\n    }else if( size>0 ){\n      fail(size);\n    }\n    close_file(file);\n  }else\n\n  /* fourth */\n  if( fourth ){\n    file = open_file(name, mode);\n    if( file==0 ){\n      stop(1);\n    }\n    if( status!=READY ){\n      fail(status);\n    }else if( size>0 ){\n      fail(size);\n    }\n    close_file(file);\n  }else\n\n  /* fifth */\n  if( fifth ){\n    file = open_file(name, mode);\n    if( file==0 ){\n      stop(1);\n    }\n    if( status!=READY ){\n      fail(status);\n    }else if( size>0 ){\n      fail(size);\n    }\n    close_file(file);\n  }\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    if( first ) {\n        file = open_file(name, mode);\n        if( file==0 ) {\n            stop(1);\n            }\n        if( status!=READY ) {\n            fail(status);\n            }\n        else if( size>0 ) {\n            fail(size);\n            }\n        close_file(file);\n        }\n    else\n\n        /* second */\n        if( second ) {\n            file = open_file(name, mode);\n            if( file==0 ) {\n                stop(1);\n                }\n            if( status!=READY ) {\n                fail(status);\n                }\n            else if( size>0 ) {\n                fail(size);\n                }\n            close_file(file);\n            }\n        else\n\n            /* third */\n            if( third ) {\n                file = open_file(name, mode);\n                if( file==0 ) {\n                    stop(1);\n                    }\n                if( status!=READY ) {\n                    fail(status);\n                    }\n                else if( size>0 ) {\n                    fail(size);\n                    }\n                close_file(file);\n                }\n            else\n\n                /* fourth */\n                if( fourth ) {\n                    file = open_file(name, mode);\n                    if( file==0 ) {\n                        stop(1);\n                        }\n                    if( status!=READY ) {\n                        fail(status);\n                        }\n                    else if( size>0 ) {\n                        fail(size);\n                        }\n                    close_file(file);\n                    }\n                else\n\n                    /* fifth */\n                    if( fifth ) {\n                        file = open_file(name, mode);\n                        if( file==0 ) {\n                            stop(1);\n                            }\n                        if( status!=READY ) {\n                            fail(status);\n                            }\n                        else if( size>0 ) {\n                            fail(size);\n                            }\n                        close_file(file);\n                        }\n    }\n",
    );
}

#[test]
fn ratliff_closes_a_case_block_loop_deep_in_a_commented_else_chain_at_its_body() {
    let input = "void f()\n{\n  if( first ){\n    switch( op ){\n      case 1: {\n        for(i=0; i<count; i++){\n          visit(i);\n        }\n        break;\n      }\n    }\n  }else\n\n  /* second */\n  if( second ){\n    switch( op ){\n      case 1: {\n        for(i=0; i<count; i++){\n          visit(i);\n        }\n        break;\n      }\n    }\n  }else\n\n  /* third */\n  if( third ){\n    switch( op ){\n      case 1: {\n        for(i=0; i<count; i++){\n          visit(i);\n        }\n        break;\n      }\n    }\n  }else\n\n  /* fourth */\n  if( fourth ){\n    switch( op ){\n      case 1: {\n        for(i=0; i<count; i++){\n          visit(i);\n        }\n        break;\n      }\n    }\n  }else\n\n  /* fifth */\n  if( fifth ){\n    switch( op ){\n      case 1: {\n        for(i=0; i<count; i++){\n          visit(i);\n        }\n        break;\n      }\n    }\n  }\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    if( first ) {\n        switch( op ) {\n            case 1: {\n                for(i=0; i<count; i++) {\n                    visit(i);\n                    }\n                break;\n                }\n            }\n        }\n    else\n\n        /* second */\n        if( second ) {\n            switch( op ) {\n                case 1: {\n                    for(i=0; i<count; i++) {\n                        visit(i);\n                        }\n                    break;\n                    }\n                }\n            }\n        else\n\n            /* third */\n            if( third ) {\n                switch( op ) {\n                    case 1: {\n                        for(i=0; i<count; i++) {\n                            visit(i);\n                            }\n                        break;\n                        }\n                    }\n                }\n            else\n\n                /* fourth */\n                if( fourth ) {\n                    switch( op ) {\n                        case 1: {\n                            for(i=0; i<count; i++) {\n                                visit(i);\n                                }\n                            break;\n                            }\n                        }\n                    }\n                else\n\n                    /* fifth */\n                    if( fifth ) {\n                        switch( op ) {\n                            case 1: {\n                                for(i=0; i<count; i++) {\n                                    visit(i);\n                                    }\n                                break;\n                                }\n                            }\n                        }\n    }\n",
    );
}

#[test]
fn a_ternary_inside_a_subscript_ends_with_its_statement() {
    // Its colon closes it: no open ternary carries into the switch after.
    let input = "static void g(int n)\n{\n  buff[flag ? 0 : size - 1] = 1;\n}\nvoid f()\n{\n  switch (opt) {\n      case Kint: {  /* signed */\n        pack(n);\n      }\n  }\n}\n";
    check(
        input,
        &["--style=gnu", "--indent-switches", "--indent-cases"],
        "static void g(int n)\n{\n    buff[flag ? 0 : size - 1] = 1;\n}\nvoid f()\n{\n    switch (opt)\n        {\n            case Kint:    /* signed */\n                {\n                    pack(n);\n                }\n        }\n}\n",
    );
}

#[test]
fn a_ternary_choosing_between_messages_keeps_its_colon_spacing() {
    // A selector colon inside a message leaves the `?` around it waiting
    // for its own colon.
    let input = "@implementation Item\n- (void)update {\n    value = ready ? [self first:1] : [self second:2];\n}\n@end\n";
    check(input, &[], input);
}

#[test]
fn an_indented_label_before_the_first_case_stands_with_the_case_labels() {
    // Statements before the first case stand in the case body, so an
    // indented label there takes one level less, as one after a case does.
    let input = "void f()\n{\n    switch (state) {\nretry:\n    case 1:\n        value = 1;\n        break;\n    case 2:\n        goto retry;\n    }\n}\n";
    check(
        input,
        &["--indent-switches", "--indent-labels"],
        "void f()\n{\n    switch (state) {\n        retry:\n        case 1:\n            value = 1;\n            break;\n        case 2:\n            goto retry;\n    }\n}\n",
    );
    check(
        input,
        &["--style=gnu", "--indent-switches", "--indent-labels"],
        "void f()\n{\n    switch (state)\n        {\n            retry:\n            case 1:\n                value = 1;\n                break;\n            case 2:\n                goto retry;\n        }\n}\n",
    );
    check(
        input,
        &["--style=whitesmith", "--indent-labels"],
        "void f()\n    {\n    switch (state)\n        {\n        retry:\n        case 1:\n            value = 1;\n            break;\n        case 2:\n            goto retry;\n        }\n    }\n",
    );
}

#[test]
fn a_case_label_continued_before_its_colon_goes_on_two_levels_past_the_label() {
    // Every split case label continues at the same distance from its own
    // column, the first one in a switch as the later ones.
    let input = "void f()\n{\n    switch (flags) {\n    case FIRST |\n        SECOND:\n        value = 1;\n        break;\n    case FIRST |\n        THIRD:\n        value = 2;\n        break;\n    }\n}\n";
    check(
        input,
        &["--indent-switches"],
        "void f()\n{\n    switch (flags) {\n        case FIRST |\n                SECOND:\n            value = 1;\n            break;\n        case FIRST |\n                THIRD:\n            value = 2;\n            break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=vtk"],
        "void f()\n{\n    switch (flags)\n        {\n        case FIRST |\n                SECOND:\n            value = 1;\n            break;\n        case FIRST |\n                THIRD:\n            value = 2;\n            break;\n        }\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (flags)\n        {\n        case FIRST |\n                SECOND:\n            value = 1;\n            break;\n        case FIRST |\n                THIRD:\n            value = 2;\n            break;\n        }\n    }\n",
    );
}

#[test]
fn a_label_block_in_an_unindented_case_block_nests_from_the_case_body() {
    // The case block's unindent moves a label's block with the statements
    // around it, its closer included.
    let input = "void f()\n{\n    switch (flags) {\n    case FIRST: {\nretry: {\n            value = 1;\n            goto retry;\n        }\n        break;\n    }\n    }\n}\n";
    check(input, &[], input);
    check(
        input,
        &["--indent-switches"],
        "void f()\n{\n    switch (flags) {\n        case FIRST: {\nretry: {\n                value = 1;\n                goto retry;\n            }\n            break;\n        }\n    }\n}\n",
    );
    check(
        input,
        &["--style=gnu"],
        "void f()\n{\n    switch (flags)\n        {\n        case FIRST:\n        {\nretry:\n            {\n                value = 1;\n                goto retry;\n            }\n            break;\n        }\n        }\n}\n",
    );
}

#[test]
fn a_case_label_continued_into_its_block_in_an_unindented_case_block_keeps_its_block_shape() {
    let input = "void f()\n{\n    switch (flags) {\n    case FIRST: {\n        switch (mode) {\n        case FIRST |\n            SECOND: {\n            value = 1;\n            break;\n        }\n        }\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    switch (flags) {\n    case FIRST: {\n        switch (mode) {\n        case FIRST |\n                SECOND: {\n                value = 1;\n                break;\n            }\n        }\n        break;\n    }\n    }\n}\n",
    );
    check(
        input,
        &["--indent-switches"],
        "void f()\n{\n    switch (flags) {\n        case FIRST: {\n            switch (mode) {\n                case FIRST |\n                        SECOND: {\n                        value = 1;\n                        break;\n                    }\n            }\n            break;\n        }\n    }\n}\n",
    );
    check(
        input,
        &["--style=gnu"],
        "void f()\n{\n    switch (flags)\n        {\n        case FIRST:\n        {\n            switch (mode)\n                {\n                case FIRST |\n                        SECOND:\n                    {\n                        value = 1;\n                        break;\n                    }\n                }\n            break;\n        }\n        }\n}\n",
    );
}

#[test]
fn a_label_block_in_an_indented_case_block_nests_from_the_case_body_when_closers_attach() {
    // Closers joining the rows before them leave the block a label opens in
    // a case block one level past the case body, as in the other styles.
    let input = "void f()\n{\n    switch (flags) {\n    case FIRST: {\nretry: {\n            value = 1;\n            goto retry;\n        }\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=lisp", "--indent-switches", "--indent-cases"],
        "void f() {\n    switch (flags) {\n        case FIRST: {\nretry: {\n                    value = 1;\n                    goto retry; }\n                break; } } }\n",
    );
    check(
        input,
        &["--style=pico", "--indent-cases"],
        "void f()\n{   switch (flags)\n    {   case FIRST:\n            {   retry:\n                {   value = 1;\n                    goto retry; }\n                break; } } }\n",
    );
}

#[test]
fn a_run_in_statement_past_the_brace_level_reaches_its_column_with_tabs() {
    // A statement before the first case stands in the case body: run into
    // the switch brace it keeps that column, with spaces or tabs alike.
    let input = "void f()\n{\n    switch (state) {\n        value = 0;\n    case 1:\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=horstmann"],
        "void f()\n{   switch (state)\n    {       value = 0;\n        case 1:\n            break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=horstmann", "--indent=tab"],
        "void f()\n{\tswitch (state)\n\t{\t\tvalue = 0;\n\t\tcase 1:\n\t\t\tbreak;\n\t}\n}\n",
    );
    check(
        input,
        &["--style=pico", "--indent=tab"],
        "void f()\n{\tswitch (state)\n\t{\t\tvalue = 0;\n\t\tcase 1:\n\t\t\tbreak; } }\n",
    );
}

#[test]
fn a_brace_after_a_directive_under_a_case_label_opens_a_plain_block_in_the_case_body() {
    // A directive between a case label and a brace leaves the brace a
    // plain block among the case's statements, whatever the case options.
    let input = "void f()\n{\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n    {\n        pop(state);\n    }\n    break;\n    case 2:\n    {\n        push(state);\n    }\n    break;\n    }\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n        {\n            pop(state);\n        }\n        break;\n    case 2:\n    {\n        push(state);\n    }\n    break;\n    }\n}\n",
    );
    check(
        input,
        &["--indent-cases"],
        "void f()\n{\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n        {\n            pop(state);\n        }\n        break;\n    case 2:\n        {\n            push(state);\n        }\n        break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    switch (state) {\n        case 1:\n#line 46 \"input.l\"\n                {\n                pop(state);\n                }\n            break;\n        case 2: {\n            push(state);\n            }\n        break;\n        }\n    }\n",
    );
    check(
        input,
        &["--style=gnu", "--indent-switches", "--indent-cases"],
        "void f()\n{\n    switch (state)\n        {\n            case 1:\n#line 46 \"input.l\"\n                {\n                    pop(state);\n                }\n                break;\n            case 2:\n                {\n                    push(state);\n                }\n                break;\n        }\n}\n",
    );
}

#[test]
fn every_directive_but_a_conditional_parts_a_case_label_from_its_brace() {
    // The kind of directive between a label and a brace does not change
    // whose block the brace opens: only a conditional leaves it the label's.
    let expected_plain = |directive: &str| {
        format!(
            "void f()\n{{\n    switch (state) {{\n    case 1:\n{directive}\n        {{\n            pop(state);\n        }}\n        break;\n    }}\n}}\n"
        )
    };
    for directive in ["#pragma once", "#define LIMIT 4", "#line 46", "#error stop"] {
        let input = format!(
            "void f()\n{{\n    switch (state) {{\n    case 1:\n{directive}\n    {{\n        pop(state);\n    }}\n    break;\n    }}\n}}\n"
        );
        check(&input, &[], &expected_plain(directive));
    }
    let input = "void f()\n{\n    switch (state) {\n    case 1:\n#ifdef TRACE\n    {\n        pop(state);\n    }\n#endif\n    break;\n    }\n}\n";
    check(input, &[], input);
}

#[test]
fn a_case_block_after_a_comment_led_label_closes_where_case_blocks_do() {
    // A comment leading the label leaves its block's closer where the
    // closers of the other case blocks stand.
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value = 1;\n        break;\n    }\n    /*case 2*/ default: {\n        if (ready) {\n            start();\n        }\n        result = 2;\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    switch (kind) {\n        case 1: {\n            value = 1;\n            break;\n            }\n        /*case 2*/ default: {\n            if (ready) {\n                start();\n                }\n            result = 2;\n            break;\n            }\n        }\n    }\n",
    );
    check(input, &[], input);
}

#[test]
fn a_statement_after_a_plain_or_macro_block_closer_breaks_as_after_a_header_block() {
    // Code after the closer of any statement block goes to its own line
    // unless one-line statements are kept.
    let input = "void f()\n{\n    {\n        value = 2;\n    } other();\n    { value = 3; } more();\n    MACRO(x) {\n        value = 4;\n    } after();\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    {\n        value = 2;\n    }\n    other();\n    {\n        value = 3;\n    }\n    more();\n    MACRO(x) {\n        value = 4;\n    }\n    after();\n}\n",
    );
    check(
        input,
        &["--keep-one-line-blocks"],
        "void f()\n{\n    {\n        value = 2;\n    }\n    other();\n    { value = 3; }\n    more();\n    MACRO(x) {\n        value = 4;\n    }\n    after();\n}\n",
    );
    check(
        input,
        &["--keep-one-line-statements"],
        "void f()\n{\n    {\n        value = 2;\n    } other();\n    {\n        value = 3;\n    } more();\n    MACRO(x) {\n        value = 4;\n    } after();\n}\n",
    );
}

#[test]
fn vtk_indents_a_bare_one_line_block_as_after_a_header() {
    // VTK leaves a kept one-line block's brace unindented in a function's
    // block and indents it within another, a header's or not; Whitesmith
    // indents both.
    let nested = "void f()\n{\n    if (z) {\n        { value = 5; }\n        if (x)\n        { value = 3; }\n    }\n    { value = 6; }\n}\n";
    check(
        nested,
        &["--style=vtk", "--keep-one-line-blocks"],
        "void f()\n{\n    if (z)\n        {\n            { value = 5; }\n        if (x)\n            { value = 3; }\n        }\n    { value = 6; }\n}\n",
    );
    let input = "void f()\n{\n    if (x)\n    { value = 3; }\n    while (y)\n    { value = 4; }\n    { value = 5; }\n    {\n        value = 6;\n    }\n}\n";
    check(
        input,
        &["--style=vtk", "--keep-one-line-blocks"],
        "void f()\n{\n    if (x)\n    { value = 3; }\n    while (y)\n    { value = 4; }\n    { value = 5; }\n        {\n        value = 6;\n        }\n}\n",
    );
    check(
        input,
        &["--style=whitesmith", "--keep-one-line-blocks"],
        "void f()\n    {\n    if (x)\n        { value = 3; }\n    while (y)\n        { value = 4; }\n        { value = 5; }\n        {\n        value = 6;\n        }\n    }\n",
    );
}

#[test]
fn an_element_block_after_a_directive_closes_like_the_ones_before_it() {
    // Every element block of the list breaks its closer off its last value,
    // a directive before it or not.
    let input = "static Command commands[] = {\n\n    { name(\"first\"),\n      set_slot,\n      NULL },\n\n#if (CACHE)\n\n    { name(\"second\"),\n      set_slot,\n      NULL },\n\n#endif\n\n    { name(\"third\"),\n      set_slot,\n      NULL },\n\n      null_command\n};\n";
    check(
        input,
        &[],
        "static Command commands[] = {\n\n    {   name(\"first\"),\n        set_slot,\n        NULL\n    },\n\n#if (CACHE)\n\n    {   name(\"second\"),\n        set_slot,\n        NULL\n    },\n\n#endif\n\n    {   name(\"third\"),\n        set_slot,\n        NULL\n    },\n\n    null_command\n};\n",
    );
}

#[test]
fn a_statement_after_a_word_headed_block_stays_on_its_closer() {
    // A word heading a block makes the block the word's, as a macro's: the
    // statement after the closer goes on from it, the brace on the word's
    // line or its own.
    let input = "void f()\n{\n    Binder {\n        bind(first);\n    } _Binder;\n    Binder\n    {\n        bind(second);\n    } _Binder;\n}\n";
    check(input, &[], input);
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    Binder\n    {\n        bind(first);\n    } _Binder;\n    Binder\n    {\n        bind(second);\n    } _Binder;\n}\n",
    );
}

#[test]
fn break_blocks_parts_a_case_labels_block_from_a_directive_after_it() {
    // The block under a case label stays the label's past a directive
    // between them: it parts from a directive after it as a case block
    // does, where a plain block among the statements does not.
    let input = "void f()\n{\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n        {\n            pop(state);\n        }\n#line 50 \"out.c\"\n        break;\n    case 2:\n        run(state);\n        {\n            push(state);\n        }\n#line 60 \"out.c\"\n        break;\n    }\n}\n";
    check(
        input,
        &["--break-blocks"],
        "void f()\n{\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n        {\n            pop(state);\n        }\n\n#line 50 \"out.c\"\n        break;\n\n    case 2:\n        run(state);\n        {\n            push(state);\n        }\n#line 60 \"out.c\"\n        break;\n    }\n}\n",
    );
}

#[test]
fn whitesmith_blocks_in_a_switch_after_a_statement_macro_stand_as_in_any_switch() {
    // A macro line without a semicolon ends its own statement: the switch
    // after it owns its body, whose blocks take the case body's columns.
    let input = "void f()\n{\n  SETUP_BEGIN\n  switch (kind)\n    {\n    case FIRST:\n#line 37 \"input.y\"\n            { release(value); }\n        break;\n    case OTHER:\n        run(value);\n        { push(value); }\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    SETUP_BEGIN\n    switch (kind)\n        {\n        case FIRST:\n#line 37 \"input.y\"\n                {\n                release(value);\n                }\n            break;\n        case OTHER:\n            run(value);\n                {\n                push(value);\n                }\n            break;\n        }\n    }\n",
    );
}

#[test]
fn a_brace_opening_an_alternative_branch_after_a_header_is_the_headers_block() {
    // The other branch of a group right after a header gives the header its
    // body: a brace there opens the header's block, not a statement of a
    // braceless body.
    let input = "void f()\n{\n  if (full)\n#if !defined OVERFLOW\n    fail();\n#else\n    {\n      grow();\n    }\n#endif\n  done();\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    if (full)\n#if !defined OVERFLOW\n        fail();\n#else\n    {\n        grow();\n    }\n#endif\n    done();\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    if (full)\n#if !defined OVERFLOW\n        fail();\n#else\n        {\n        grow();\n        }\n#endif\n    done();\n    }\n",
    );
    check(
        "void f()\n{\n  while (busy)\n    {\n      if (full)\n#if !defined OVERFLOW\n        fail();\n#else\n        {\n          grow();\n        }\n#endif\n    }\n}\n",
        &["--style=gnu"],
        "void f()\n{\n    while (busy)\n        {\n            if (full)\n#if !defined OVERFLOW\n                fail();\n#else\n                {\n                    grow();\n                }\n#endif\n        }\n}\n",
    );
}

#[test]
fn indenting_define_bodies_leaves_a_conditional_between_a_case_and_its_brace_transparent() {
    // Only a define's body follows that option: the brace after a
    // conditional still opens the case's block, its statements after it.
    let input = "void f()\n{\n    switch (state) {\n    case 2:\n#ifdef TRACE\n    {\n        push(state);\n    }\n#endif\n    break;\n    }\n}\n";
    check(input, &["--style=kr"], input);
    check(input, &["--style=kr", "--indent-preproc-define"], input);
}

#[test]
fn a_class_heads_continued_base_list_indents_with_tabs() {
    // A class head's continuation rows stand a level in, which tab indents
    // write as a tab, at file scope or in a function.
    let input = "class Derived : public Base,\n    private Other\n{\n};\nclass Third :\n    public Base\n{\n};\nvoid f()\n{\n    struct Local : Base,\n        Other {\n    };\n}\n";
    check(
        input,
        &["--indent=tab"],
        "class Derived : public Base,\n\tprivate Other\n{\n};\nclass Third :\n\tpublic Base\n{\n};\nvoid f()\n{\n\tstruct Local : Base,\n\t\tOther {\n\t};\n}\n",
    );
}

#[test]
fn break_blocks_pads_an_attributed_header_block_like_any_header_block() {
    // An attribute between a header and its brace leaves the block the
    // header's: break-blocks parts it from the statement after.
    let input = "void f()\n{\n    if (ready) [[likely]] {\n        value = 1;\n    }\n    done();\n    if (ready) {\n        value = 2;\n    }\n    done();\n}\n";
    check(
        input,
        &["--break-blocks"],
        "void f()\n{\n    if (ready) [[likely]] {\n        value = 1;\n    }\n\n    done();\n\n    if (ready) {\n        value = 2;\n    }\n\n    done();\n}\n",
    );
}

#[test]
fn break_blocks_parts_a_case_block_from_the_statement_after_it_but_its_break() {
    // A case's block is a header's block: what follows it is padded, save
    // the break that ends the case with it.
    let input = "void f()\n{\n    switch (kind) {\n    case 1:\n        if (x) {\n            y();\n        }\n        break;\n    case 2:\n    {\n        y();\n    }\n    break;\n    case 3:\n    {\n        y();\n    }\n    z();\n    break;\n    }\n    while (x) {\n        y();\n    }\n    return;\n}\n";
    check(
        input,
        &["--break-blocks"],
        "void f()\n{\n    switch (kind) {\n    case 1:\n        if (x) {\n            y();\n        }\n\n        break;\n\n    case 2:\n    {\n        y();\n    }\n    break;\n\n    case 3:\n    {\n        y();\n    }\n\n    z();\n    break;\n    }\n\n    while (x) {\n        y();\n    }\n\n    return;\n}\n",
    );
}

#[test]
fn a_macro_built_function_heads_parameters_are_declarations() {
    // `EXPORT(type, name)(params) {` heads a function body: its parameters
    // declare pointers, which pad-oper leaves to the pointer options.
    let input = "EXPORT(void, reset)(Item *item)\n{\n    clear(item);\n}\nEXPORT(int, count)(Item *item, char **names)\n{\n    return 0;\n}\n";
    check(input, &["--pad-oper"], input);
    check(
        input,
        &["--pad-oper", "--align-pointer=type"],
        "EXPORT(void, reset)(Item* item)\n{\n    clear(item);\n}\nEXPORT(int, count)(Item* item, char** names)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn a_function_pointer_declarator_with_calling_conventions_holds_a_pointer() {
    // Words before the star of a declarator group are its calling
    // convention: the star declares, whatever case the name takes.
    let input = "typedef BOOL (PASCAL FAR *LPFN_ACCEPT)(\n    int value,\n    int other);\n";
    check(input, &["--pad-oper"], input);
    check(
        input,
        &["--pad-oper", "--align-pointer=type"],
        "typedef BOOL (PASCAL FAR* LPFN_ACCEPT)(\n    int value,\n    int other);\n",
    );
}

#[test]
fn break_return_type_splits_the_name_line_of_a_head_past_directives() {
    // The return type before the name on its line breaks off, whatever
    // directives stand in the head or between it and a macro before it.
    let input = "#ifndef CORE\nMACRO_INIT\n#endif\n\nstatic void third(\n    int v\n){\n    use(v);\n}\nstatic\n#ifdef X\ninline\n#endif\nint first(void)\n{\n    return 0;\n}\n";
    check(
        input,
        &["--style=kr", "--break-return-type"],
        "#ifndef CORE\nMACRO_INIT\n#endif\n\nstatic void\nthird(\n    int v\n)\n{\n    use(v);\n}\nstatic\n#ifdef X\ninline\n#endif\nint\nfirst(void)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn an_indented_label_heading_a_braceless_body_stands_a_level_before_it() {
    // The label is the header's body: it stands a level less than the
    // statement it labels, as any indented label does.
    let input = "void f()\n{\n    if (ready)\nretry:\n        value -= step;\n    done();\n}\n";
    check(
        input,
        &["--indent-labels"],
        "void f()\n{\n    if (ready)\n    retry:\n        value -= step;\n    done();\n}\n",
    );
    check(
        "void f()\n{\n    while (busy) {\n        if (ready)\nretry:\n            value -= step;\n    }\n}\n",
        &["--indent-labels"],
        "void f()\n{\n    while (busy) {\n        if (ready)\n        retry:\n            value -= step;\n    }\n}\n",
    );
}

#[test]
fn attach_return_type_leaves_a_head_after_a_conditional_macro_split() {
    // A macro word left in a conditional before the head reads as the start
    // of its statement: the head is not joined, while a plain one is.
    let input = "#if defined(FAST)\nMACRO_NOINLINE\n#endif\nstatic bool\nfixup(Item *item) {\n    return true;\n}\nstatic bool\nplain(Item *item) {\n    return true;\n}\n";
    check(
        input,
        &["--style=1tbs", "--attach-return-type"],
        "#if defined(FAST)\nMACRO_NOINLINE\n#endif\nstatic bool\nfixup(Item *item)\n{\n    return true;\n}\nstatic bool plain(Item *item)\n{\n    return true;\n}\n",
    );
}

#[test]
fn a_comment_after_a_case_block_stands_in_the_case_body_before_a_statement() {
    let call = "void f(int x)\n{\n    switch (x) {\n    case 1: {\n        a();\n        break;\n    }\n    /* comment */\n    b();\n    default:\n        c();\n    }\n}\n";
    let exit = "void f(int x)\n{\n    switch (x) {\n    case 1: {\n        a();\n    }\n    /* comment */\n    break;\n    default:\n        c();\n    }\n}\n";
    for (args, input, expected) in [
        (&["--style=kr"][..], call, call.replace("    /* comment */", "        /* comment */")),
        (&["--style=kr"][..], exit, exit.replace("    /* comment */", "        /* comment */")),
        (
            &["--style=allman", "-S"][..],
            call,
            "void f(int x)\n{\n    switch (x)\n    {\n        case 1:\n        {\n            a();\n            break;\n        }\n            /* comment */\n        b();\n        default:\n            c();\n    }\n}\n".to_string(),
        ),
        (
            &["--style=kr", "-S"][..],
            call,
            "void f(int x)\n{\n    switch (x) {\n        case 1: {\n            a();\n            break;\n        }\n            /* comment */\n        b();\n        default:\n            c();\n    }\n}\n".to_string(),
        ),
        (
            &["--style=lisp", "-S"][..],
            exit,
            "void f(int x) {\n    switch (x) {\n        case 1: {\n            a(); }\n            /* comment */\n        break;\n        default:\n            c(); } }\n".to_string(),
        ),
        (
            &["--style=gnu"][..],
            exit,
            "void f(int x)\n{\n    switch (x)\n        {\n        case 1:\n        {\n            a();\n        }\n            /* comment */\n        break;\n        default:\n            c();\n        }\n}\n".to_string(),
        ),
        (
            &["--style=whitesmith"][..],
            call,
            "void f(int x)\n    {\n    switch (x)\n        {\n        case 1:\n            {\n            a();\n            break;\n            }\n            /* comment */\n        b();\n        default:\n            c();\n        }\n    }\n".to_string(),
        ),
        (
            &["--style=horstmann"][..],
            exit,
            "void f(int x)\n{   switch (x)\n    {   case 1:\n        {   a();\n        }\n            /* comment */\n        break;\n        default:\n            c();\n    }\n}\n".to_string(),
        ),
        (
            &["--style=ratliff"][..],
            call,
            "void f(int x) {\n    switch (x) {\n        case 1: {\n            a();\n            break;\n            }\n            /* comment */\n        b();\n        default:\n            c();\n        }\n    }\n".to_string(),
        ),
        (
            &["--style=pico"][..],
            exit,
            "void f(int x)\n{   switch (x)\n    {   case 1:\n        {   a(); }\n            /* comment */\n        break;\n        default:\n            c(); } }\n".to_string(),
        ),
        (
            &["--style=lisp"][..],
            call,
            "void f(int x) {\n    switch (x) {\n    case 1: {\n        a();\n        break; }\n        /* comment */\n    b();\n    default:\n        c(); } }\n".to_string(),
        ),
        // Indented cases keep the statement in the case body with it.
        (
            &["--style=ratliff", "-K"][..],
            call,
            "void f(int x) {\n    switch (x) {\n        case 1: {\n                a();\n                break;\n                }\n            /* comment */\n            b();\n        default:\n            c();\n        }\n    }\n".to_string(),
        ),
    ] {
        check(input, args, &expected);
        check(&expected, args, &expected);
    }
}

#[test]
fn a_row_of_comments_after_a_case_block_stands_at_the_next_label_as_comment_lines_do() {
    let one_row = "void f(int x)\n{\n    switch (x) {\n    case 1: {\n        a();\n    }\n    /* first */ /* second */\n    case 2:\n        b();\n    }\n}\n";
    let two_rows = "void f(int x)\n{\n    switch (x) {\n    case 1: {\n        a();\n    }\n    /* first */\n    /* second */\n    case 2:\n        b();\n    }\n}\n";
    check(one_row, &["--style=kr"], one_row);
    check(two_rows, &["--style=kr"], two_rows);
}

#[test]
fn statements_after_a_commented_row_past_a_case_block_stand_where_the_row_before_it_would() {
    let input = "void f(int x)\n{\n    switch (x) {\n    case 1: {\n        a();\n    }\n    b(); /* note */\n    c();\n    break;\n    default:\n        d();\n    }\n}\n";
    for (args, expected) in [
        (
            &["--style=kr"][..],
            "void f(int x)\n{\n    switch (x) {\n    case 1: {\n        a();\n    }\n        b(); /* note */\n    c();\n    break;\n    default:\n        d();\n    }\n}\n",
        ),
        (
            &["--style=allman"][..],
            "void f(int x)\n{\n    switch (x)\n    {\n    case 1:\n    {\n        a();\n    }\n        b(); /* note */\n    c();\n    break;\n    default:\n        d();\n    }\n}\n",
        ),
        (
            &["--style=whitesmith", "-S"][..],
            "void f(int x)\n    {\n    switch (x)\n        {\n        case 1:\n            {\n            a();\n            }\n            b(); /* note */\n        c();\n        break;\n        default:\n            d();\n        }\n    }\n",
        ),
        (
            &["--style=gnu"][..],
            "void f(int x)\n{\n    switch (x)\n        {\n        case 1:\n        {\n            a();\n        }\n            b(); /* note */\n        c();\n        break;\n        default:\n            d();\n        }\n}\n",
        ),
        (
            &["--style=horstmann"][..],
            "void f(int x)\n{   switch (x)\n    {   case 1:\n        {   a();\n        }\n            b(); /* note */\n        c();\n        break;\n        default:\n            d();\n    }\n}\n",
        ),
        (
            &["--style=pico"][..],
            "void f(int x)\n{   switch (x)\n    {   case 1:\n        {   a(); }\n            b(); /* note */\n        c();\n        break;\n        default:\n            d(); } }\n",
        ),
        (
            &["--style=vtk"][..],
            "void f(int x)\n{\n    switch (x)\n        {\n        case 1:\n            {\n            a();\n            }\n            b(); /* note */\n        c();\n        break;\n        default:\n            d();\n        }\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn vtk_sets_an_initializer_brace_after_a_struct_closer_in_a_case_block_at_the_closer() {
    let input = "void f(int op)\n{\n    if (op) {\n        static const struct {\n            int z;\n        } kw[] = {\n            { 1 },\n        };\n    }\n    switch (op) {\n    case 1: {\n        static const struct {\n            int z;\n        } kw[] = {\n            { 1 },\n        };\n        break;\n    }\n    }\n}\n";
    let expected = "void f(int op)\n{\n    if (op)\n        {\n        static const struct\n            {\n            int z;\n            } kw[] =\n            {\n                { 1 },\n            };\n        }\n    switch (op)\n        {\n        case 1:\n            {\n            static const struct\n                {\n                int z;\n                } kw[] =\n                {\n                    { 1 },\n                };\n            break;\n            }\n        }\n}\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
}

#[test]
fn an_else_switch_kept_on_its_if_line_indents_its_first_label_as_the_rest() {
    let input = "void f(int x)\n{\n    if (a) b(); else switch (x) {\n    case 1:\n        c();\n        break;\n    }\n    g();\n}\n";
    for (args, expected) in [
        (
            &["--style=kr", "-S", "-o"][..],
            "void f(int x)\n{\n    if (a) b(); else switch (x) {\n            case 1:\n                c();\n                break;\n        }\n    g();\n}\n",
        ),
        (
            &["--style=ratliff", "-o"][..],
            "void f(int x) {\n    if (a) b(); else switch (x) {\n            case 1:\n                c();\n                break;\n            }\n    g();\n    }\n",
        ),
        (
            &["--style=kr", "-o"][..],
            "void f(int x)\n{\n    if (a) b(); else switch (x) {\n        case 1:\n            c();\n            break;\n        }\n    g();\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn a_broken_while_closing_a_do_chained_after_else_stands_at_the_else_body() {
    let input = "void f(int x)\n{\n    if (a) b(); else do {\n        c();\n    } while (x);\n    g();\n}\n";
    for (style, expected) in [
        (
            "--style=allman",
            "void f(int x)\n{\n    if (a) b();\n    else do\n        {\n            c();\n        }\n        while (x);\n    g();\n}\n",
        ),
        (
            "--style=whitesmith",
            "void f(int x)\n    {\n    if (a) b();\n    else do\n            {\n            c();\n            }\n        while (x);\n    g();\n    }\n",
        ),
        (
            "--style=gnu",
            "void f(int x)\n{\n    if (a) b();\n    else do\n            {\n                c();\n            }\n        while (x);\n    g();\n}\n",
        ),
        (
            "--style=ratliff",
            "void f(int x) {\n    if (a) b();\n    else do {\n            c();\n            }\n        while (x);\n    g();\n    }\n",
        ),
        (
            "--style=horstmann",
            "void f(int x)\n{   if (a) b();\n    else do\n        {   c();\n        }\n        while (x);\n    g();\n}\n",
        ),
        (
            "--style=vtk",
            "void f(int x)\n{\n    if (a) b();\n    else do\n            {\n            c();\n            }\n        while (x);\n    g();\n}\n",
        ),
        (
            "--style=stroustrup",
            "void f(int x)\n{\n    if (a) b();\n    else do {\n            c();\n        }\n        while (x);\n    g();\n}\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

#[test]
fn break_one_line_headers_keeps_a_run_in_if_line_whole_whatever_follows_its_else() {
    // Run-in styles keep `if (x) a(); else ...` on its line, an `else`
    // heading a block as much as an `else` with a statement.
    let input = "void f()\n{\n    if (ready) helper(); else other();\n    if (ready) helper(); else switch (state) {\n    case 1:\n        first();\n        break;\n    }\n    if (ready) helper(); else for (;;) {\n        first();\n    }\n    g();\n}\n";
    for (style, expected) in [
        (
            "--style=lisp",
            "void f() {\n    if (ready) helper(); else other();\n    if (ready) helper(); else switch (state) {\n        case 1:\n            first();\n            break; }\n    if (ready) helper(); else for (;;) {\n            first(); }\n    g(); }\n",
        ),
        (
            "--style=pico",
            "void f()\n{   if (ready) helper(); else other();\n    if (ready) helper(); else switch (state)\n        {   case 1:\n                first();\n                break; }\n    if (ready) helper(); else for (;;)\n        {   first(); }\n    g(); }\n",
        ),
    ] {
        check(input, &[style, "--break-one-line-headers"], expected);
        check(input, &[style], expected);
    }
    // So do kept one-line statements in any style.
    let kept = "void f()\n{\n    if (ready) helper(); else other();\n    if (ready) helper(); else switch (state) {\n        case 1:\n            first();\n            break;\n        }\n    if (ready) helper(); else for (;;) {\n            first();\n        }\n    g();\n}\n";
    check(
        input,
        &["--style=kr", "-o", "--break-one-line-headers"],
        kept,
    );
    check(input, &["--style=kr", "-o"], kept);
}

#[test]
fn break_blocks_all_keeps_an_else_with_its_one_line_if() {
    let input = "void f()\n{\n    for (;;) { if (ready) break; else continue; }\n}\n";
    check(
        input,
        &[
            "--style=kr",
            "--keep-one-line-statements",
            "--break-blocks=all",
        ],
        "void f()\n{\n    for (;;) {\n        if (ready) break;\n        else continue;\n    }\n}\n",
    );
}

#[test]
fn run_in_styles_keep_the_source_gap_before_a_closer_on_its_statement_line() {
    let input = "void f()\n{\n    if (ready) {\n        value=0;\t}\n    if (ready) {\n        value=0; \t}\n    if (ready) {\n        value=0;\t\t}\n    if (ready) {\n        value=0;  }\n}\n";
    for (style, expected) in [
        (
            "--style=lisp",
            "void f() {\n    if (ready) {\n        value=0;\t}\n    if (ready) {\n        value=0; \t}\n    if (ready) {\n        value=0;\t\t}\n    if (ready) {\n        value=0;  } }\n",
        ),
        (
            "--style=pico",
            "void f()\n{   if (ready)\n    {   value=0;\t}\n    if (ready)\n    {   value=0; \t}\n    if (ready)\n    {   value=0;\t\t}\n    if (ready)\n    {   value=0;  } }\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

#[test]
fn a_while_after_a_run_in_do_body_of_an_else_on_its_if_line_stands_at_the_else_body() {
    let input = "void f()\n{\n    if (ready) helper(); else do {\n        first();\n    } while (state);\n}\n";
    for (style, expected) in [
        (
            "--style=lisp",
            "void f() {\n    if (ready) helper(); else do {\n            first(); }\n        while (state); }\n",
        ),
        (
            "--style=pico",
            "void f()\n{   if (ready) helper(); else do\n        {   first(); }\n        while (state); }\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

#[test]
fn an_added_block_of_a_header_chained_after_else_indents_past_the_else_body() {
    let input = "void f(void)\n{\n    if (a) {\n        b();\n    }\n    else for (; pre>0; pre--) *c=0; // note\n    g();\n}\n";
    let expected = "void f(void)\n{\n    if (a) {\n        b();\n    } else for (; pre>0; pre--) {\n            *c=0;    // note\n        }\n    g();\n}\n";
    check(input, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

#[test]
fn a_block_of_a_do_run_in_after_an_else_on_its_if_line_closes_at_the_else_body() {
    let input = "void f()\n{\n    if (ready) helper(); else do\n    {\n        first();\n    }\n    while (state);\n    if (ready) helper(); else for (;;)\n    {\n        first();\n    }\n    g();\n}\n";
    let attached = "void f()\n{\n    if (ready) helper(); else do {\n            first();\n        } while (state);\n    if (ready) helper(); else for (;;) {\n            first();\n        }\n    g();\n}\n";
    for (style, expected) in [
        ("--style=kr", attached.to_string()),
        ("--style=java", attached.replacen("()\n{", "() {", 1)),
        (
            "--style=allman",
            "void f()\n{\n    if (ready) helper(); else do\n        {\n            first();\n        }\n        while (state);\n    if (ready) helper(); else for (;;)\n        {\n            first();\n        }\n    g();\n}\n".to_string(),
        ),
    ] {
        check(input, &[style, "-o"], &expected);
        check(&expected, &[style, "-o"], &expected);
    }
}

#[test]
fn keeping_one_line_statements_keeps_an_if_line_ending_in_its_else_whole() {
    // Its `else` stays on the kept line whether the body follows on that
    // line or the next, as the line the style formats it into does.
    let input = "void f()\n{\n    if (ready) helper(); else\n    {\n        first();\n    }\n    if (ready) helper(); else\n        first();\n    if (ready) helper(); else {\n        first();\n    }\n    g();\n}\n";
    for (style, expected) in [
        (
            "--style=kr",
            "void f()\n{\n    if (ready) helper(); else {\n        first();\n    }\n    if (ready) helper(); else\n        first();\n    if (ready) helper(); else {\n        first();\n    }\n    g();\n}\n",
        ),
        (
            "--style=allman",
            "void f()\n{\n    if (ready) helper(); else\n    {\n        first();\n    }\n    if (ready) helper(); else\n        first();\n    if (ready) helper(); else\n    {\n        first();\n    }\n    g();\n}\n",
        ),
    ] {
        check(input, &[style, "-o"], expected);
        check(expected, &[style, "-o"], expected);
    }
}

#[test]
fn a_one_line_block_removed_from_a_header_run_in_after_an_else_nests_past_both() {
    let input = "void f()\n{\n    if (ready) helper(); else for (;;)\n    {   first(); }\n    if (ready) helper(); else while (ready)\n    {\n        first();\n    }\n    g();\n}\n";
    let expected = "void f()\n{\n    if (ready) helper();\n    else for (;;)\n            first();\n    if (ready) helper();\n    else while (ready)\n            first();\n    g();\n}\n";
    for style in ["--style=kr", "--style=allman"] {
        check(input, &[style, "--remove-braces"], expected);
        check(expected, &[style, "--remove-braces"], expected);
    }
}

#[test]
fn a_switch_chained_after_an_else_kept_past_a_removed_block_lays_out_at_the_else_body() {
    let input = "void f(int x)\n{\n    if (a) { b(); } else switch (x) {\n    case 1:\n        c();\n        break;\n    }\n    g();\n}\n";
    for (style, expected) in [
        (
            "--style=kr",
            "void f(int x)\n{\n    if (a)\n        b();   else switch (x) {\n        case 1:\n            c();\n            break;\n        }\n    g();\n}\n",
        ),
        (
            "--style=allman",
            "void f(int x)\n{\n    if (a)\n        b();   else switch (x)\n        {\n        case 1:\n            c();\n            break;\n        }\n    g();\n}\n",
        ),
        (
            "--style=whitesmith",
            "void f(int x)\n    {\n    if (a)\n        b();   else switch (x)\n            {\n            case 1:\n                c();\n                break;\n            }\n    g();\n    }\n",
        ),
    ] {
        check(input, &[style, "--remove-braces", "-o"], expected);
        check(expected, &[style, "--remove-braces", "-o"], expected);
    }
}

#[test]
fn a_header_chained_after_an_else_on_its_ifs_body_line_nests_from_the_if() {
    let input = "void f(int x)\n{\n    if (a)\n        b(); else for (;;) {\n            c();\n        }\n    if (a)\n        b(); else switch (x) {\n        case 1:\n            c();\n            break;\n        }\n    g();\n}\n";
    for (args, expected) in [
        (&["--style=kr", "-o"][..], input.to_string()),
        (
            &["--style=allman", "-o"][..],
            "void f(int x)\n{\n    if (a)\n        b(); else for (;;)\n        {\n            c();\n        }\n    if (a)\n        b(); else switch (x)\n        {\n        case 1:\n            c();\n            break;\n        }\n    g();\n}\n".to_string(),
        ),
        (
            &["--style=whitesmith", "-o"][..],
            "void f(int x)\n    {\n    if (a)\n        b(); else for (;;)\n            {\n            c();\n            }\n    if (a)\n        b(); else switch (x)\n            {\n            case 1:\n                c();\n                break;\n            }\n    g();\n    }\n".to_string(),
        ),
        (
            &["--style=kr", "-o", "-S"][..],
            "void f(int x)\n{\n    if (a)\n        b(); else for (;;) {\n            c();\n        }\n    if (a)\n        b(); else switch (x) {\n            case 1:\n                c();\n                break;\n        }\n    g();\n}\n".to_string(),
        ),
    ] {
        check(input, args, &expected);
        check(&expected, args, &expected);
    }
}

#[test]
fn a_comment_after_an_added_one_line_block_keeps_its_column_through_unpadding_in_lisp() {
    let input = "void f()\n{\n    if (ready) value=0;   // note\n}\n";
    for (style, expected) in [
        (
            "--style=kr",
            "void f()\n{\n    if(ready) { value=0; }    // note\n}\n",
        ),
        (
            "--style=lisp",
            "void f() {\n    if(ready) { value=0; }    // note\n}\n",
        ),
    ] {
        let args = [
            style,
            "--keep-one-line-blocks",
            "--add-braces",
            "--unpad-paren",
        ];
        check(input, &args, expected);
    }
}

#[test]
fn break_else_ifs_leaves_a_kept_one_line_statement_chain_whole() {
    let input = "void f()\n{\n    if (ready) value=0; else if (other) result=0; else result=1;\n    if (ready) { value=0; } else if (other) { result=0; } else { result=1; }\n}\n";
    check(
        input,
        &["--style=kr", "-o", "--break-elseifs"],
        "void f()\n{\n    if (ready) value=0; else if (other) result=0; else result=1;\n    if (ready) {\n        value=0;\n    } else\n        if (other) {\n            result=0;\n        } else {\n            result=1;\n        }\n}\n",
    );
    check(
        input,
        &["--style=kr", "-o", "--break-elseifs", "--remove-braces"],
        "void f()\n{\n    if (ready) value=0; else if (other) result=0; else result=1;\n    if (ready)\n        value=0;   else if (other)\n        result=0;   else\n        result=1;\n}\n",
    );
}

#[test]
fn breaking_one_line_headers_keeps_a_closing_header_with_what_goes_on_after_it() {
    // Kept one-line statements keep `else` and `while` on the statement's
    // line while code goes on after them there; a brace the style breaks
    // off leaves the `else` ending the line, which parts it.
    let input = "void f(int x)\n{\n    if (a)\n        b(); else c();\n    if (a)\n        b(); else {\n        c();\n    }\n    if (a)\n        b(); else for (;;) {\n        c();\n    }\n    do\n        b(); while (x);\n}\n";
    for (style, expected) in [
        (
            "--style=kr",
            "void f(int x)\n{\n    if (a)\n        b(); else c();\n    if (a)\n        b(); else {\n        c();\n    }\n    if (a)\n        b(); else for (;;) {\n            c();\n        }\n    do\n        b(); while (x);\n}\n",
        ),
        (
            "--style=allman",
            "void f(int x)\n{\n    if (a)\n        b(); else c();\n    if (a)\n        b();\n    else\n    {\n        c();\n    }\n    if (a)\n        b(); else for (;;)\n        {\n            c();\n        }\n    do\n        b(); while (x);\n}\n",
        ),
    ] {
        check(input, &[style, "-o", "--break-one-line-headers"], expected);
        check(
            expected,
            &[style, "-o", "--break-one-line-headers"],
            expected,
        );
    }
}

#[test]
fn a_run_in_closer_after_a_comment_leaves_no_gap_behind() {
    // The closer cannot join a row a comment ends, so the gap before it
    // goes with it.
    let input = "static void f(int *p){ g(p); /*none*/ }\n";
    for (style, expected) in [
        (
            "--style=lisp",
            "static void f(int *p) {\n    g(p); /*none*/\n}\n",
        ),
        ("--style=pico", "static void f(int *p) { g(p); /*none*/ }\n"),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

// An added brace goes right before its statement, after a comment the
// statement's line starts with, as after a comment line.
#[test]
fn added_braces_open_after_a_comment_leading_the_statement_line() {
    let input =
        "void f()\n{\n    if (b)\n        /* c */ x();\n    else\n        /* d */ y();\n}\n";
    let cases: [(&[&str], &str); 10] = [
        (
            &["-j"],
            "void f()\n{\n    if (b)\n        /* c */ {\n        x();\n    }\n    else\n        /* d */ {\n        y();\n    }\n}\n",
        ),
        (
            &["-j", "--style=kr"],
            "void f()\n{\n    if (b)\n        /* c */ {\n        x();\n    } else\n        /* d */ {\n        y();\n    }\n}\n",
        ),
        (
            &["-j", "--style=allman"],
            "void f()\n{\n    if (b)\n        /* c */\n    {\n        x();\n    }\n    else\n        /* d */\n    {\n        y();\n    }\n}\n",
        ),
        (
            &["-j", "--style=gnu"],
            "void f()\n{\n    if (b)\n        /* c */\n        {\n            x();\n        }\n    else\n        /* d */\n        {\n            y();\n        }\n}\n",
        ),
        (
            &["-j", "--style=whitesmith"],
            "void f()\n    {\n    if (b)\n        /* c */\n        {\n        x();\n        }\n    else\n        /* d */\n        {\n        y();\n        }\n    }\n",
        ),
        (
            &["-j", "--style=lisp"],
            "void f() {\n    if (b)\n        /* c */ {\n        x(); }\n    else\n        /* d */ {\n        y(); } }\n",
        ),
        (
            &["-j", "--style=horstmann"],
            "void f()\n{   if (b)\n        /* c */\n    {   x();\n    }\n    else\n        /* d */\n    {   y();\n    }\n}\n",
        ),
        (
            &["-j", "--style=ratliff"],
            "void f() {\n    if (b)\n        /* c */ {\n        x();\n        }\n    else\n        /* d */ {\n        y();\n        }\n    }\n",
        ),
        (
            &["-j", "--style=vtk"],
            "void f()\n{\n    if (b)\n        /* c */\n        {\n        x();\n        }\n    else\n        /* d */\n        {\n        y();\n        }\n}\n",
        ),
        (
            &["-j", "--style=1tbs"],
            "void f()\n{\n    if (b)\n        /* c */ {\n        x();\n    } else\n        /* d */ {\n        y();\n    }\n}\n",
        ),
    ];
    for (args, expected) in cases {
        check(input, args, expected);
        check(expected, args, expected);
    }
    // A written brace there closes at its header as well.
    let written = "void f()\n{\n    if (b)\n        /* c */ {\n        x();\n    }\n}\n";
    check(written, &["--style=kr"], written);
}

// An added brace goes after every comment leading the statement's line, in
// attached styles as in broken ones.
#[test]
fn added_braces_open_after_all_comments_leading_the_statement_line() {
    let input = "void f()\n{\n    while (b)\n        /* c */ /* d */ x();\n}\n";
    let attached =
        "void f()\n{\n    while (b)\n        /* c */ /* d */ {\n        x();\n    }\n}\n";
    let broken =
        "void f()\n{\n    while (b)\n        /* c */ /* d */\n    {\n        x();\n    }\n}\n";
    for (args, expected) in [
        (&["-j", "--style=kr"][..], attached),
        (&["-j", "--style=allman"][..], broken),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

// Pico gives a block added after a comment leading the statement's line the
// run-in shape of its other added blocks.
#[test]
fn pico_keeps_a_block_added_after_a_comment_leading_its_line_on_that_line() {
    let input = "void f()\n{\n    if (b)\n        /* c */ x();\n    if (b)\n        x();\n}\n";
    let expected = "void f()\n{   if (b)\n        /* c */ { x(); }\n    if (b)\n    {   x(); } }\n";
    check(input, &["-j", "--style=pico"], expected);
    check(expected, &["-j", "--style=pico"], expected);
}

// A macro call standing for a header's condition ends the header for added
// braces as it does for breaking one-line headers.
#[test]
fn added_braces_wrap_the_body_of_a_header_whose_condition_is_a_macro_call() {
    let input = "void f()\n{\n    if EQ(\"\") return 0;\n    else if EQ(\"a\") value = 1;\n    if EQ(\"b\")\n        value = 2;\n    value = 3;\n    while MORE(x)\n        value = 4;\n}\n";
    for (args, expected) in [
        (
            &["-j"][..],
            "void f()\n{\n    if EQ(\"\") {\n        return 0;\n    }\n    else if EQ(\"a\") {\n        value = 1;\n    }\n    if EQ(\"b\") {\n        value = 2;\n    }\n    value = 3;\n    while MORE(x) {\n        value = 4;\n    }\n}\n",
        ),
        (
            &["--style=kr", "-j"][..],
            "void f()\n{\n    if EQ(\"\") {\n        return 0;\n    } else if EQ(\"a\") {\n        value = 1;\n    }\n    if EQ(\"b\") {\n        value = 2;\n    }\n    value = 3;\n    while MORE(x) {\n        value = 4;\n    }\n}\n",
        ),
        (
            &["--style=allman", "-j"][..],
            "void f()\n{\n    if EQ(\"\")\n    {\n        return 0;\n    }\n    else if EQ(\"a\")\n    {\n        value = 1;\n    }\n    if EQ(\"b\")\n    {\n        value = 2;\n    }\n    value = 3;\n    while MORE(x)\n    {\n        value = 4;\n    }\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

// Loop headers take a macro call for their condition as `if` does.
#[test]
fn loop_headers_take_a_macro_call_for_their_condition() {
    let input = "void f()\n{\n    while MORE(x) value = 5;\n    for EACH(x) value = 4;\n    value = 6;\n}\n";
    for (args, expected) in [
        (
            &["-j"][..],
            "void f()\n{\n    while MORE(x) {\n        value = 5;\n    }\n    for EACH(x) {\n        value = 4;\n    }\n    value = 6;\n}\n",
        ),
        (
            &["-xb"][..],
            "void f()\n{\n    while MORE(x)\n        value = 5;\n    for EACH(x)\n        value = 4;\n    value = 6;\n}\n",
        ),
        (
            &["-J"][..],
            "void f()\n{\n    while MORE(x) { value = 5; }\n    for EACH(x) { value = 4; }\n    value = 6;\n}\n",
        ),
        (
            &["--style=allman", "-j"][..],
            "void f()\n{\n    while MORE(x)\n    {\n        value = 5;\n    }\n    for EACH(x)\n    {\n        value = 4;\n    }\n    value = 6;\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
    let next_line = "void f()\n{\n    while MORE(x)\n        value = 4;\n}\n";
    check(
        next_line,
        &["-J"],
        "void f()\n{\n    while MORE(x)\n    { value = 4; }\n}\n",
    );
}

// An else broken off the closer of braces added on its line leaves its
// comment where the joined line had it, as after a written closer.
#[test]
fn an_else_broken_off_an_added_closer_keeps_its_comment_column() {
    let input = "void f()\n{\n    if (a) x(); else // note\n        y();\n    if (a) x();   else // note\n        y();\n    if (a) x();else // note\n    {\n        y();\n    }\n}\n";
    for (args, expected) in [
        (
            &["--style=allman", "-j"][..],
            "void f()\n{\n    if (a)\n    {\n        x();\n    }\n    else   // note\n    {\n        y();\n    }\n    if (a)\n    {\n        x();\n    }\n    else     // note\n    {\n        y();\n    }\n    if (a)\n    {\n        x();\n    }\n    else  // note\n    {\n        y();\n    }\n}\n",
        ),
        (
            &["--style=horstmann", "-j"][..],
            "void f()\n{   if (a)\n    {   x();\n    }\n    else   // note\n    {   y();\n    }\n    if (a)\n    {   x();\n    }\n    else     // note\n    {   y();\n    }\n    if (a)\n    {   x();\n    }\n    else  // note\n    {   y();\n    }\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
    // A comment that code follows on its line keeps its gap.
    check(
        "void f()\n{\n    if (a) x(); else /* note */ y();\n}\n",
        &["--style=allman", "-j"],
        "void f()\n{\n    if (a)\n    {\n        x();\n    }\n    else /* note */\n    {\n        y();\n    }\n}\n",
    );
}

// A brace after a comment that leads its line takes the body of its header,
// as a brace leading a line after that comment does.
#[test]
fn a_brace_after_a_comment_leading_its_line_after_an_else_holds_the_else_body() {
    let input = "void f()\n{\n    if (a) {\n        x();\n    } else\n\n        /* note */{\n        y();\n    }\n    z();\n}\n";
    for args in [&[][..], &["--style=kr"][..]] {
        check(input, args, input);
    }
    // An else in an `if` a comment after an outer `else` nests closes its
    // block at its own line.
    let nested = "void f()\n{\n    if (a) {\n        x();\n    } else\n\n    /* c */\n    if (b) {\n        y();\n    } else\n\n    /* error */{\n        z();\n    }\n    w();\n}\n";
    let attached = "void f()\n{\n    if (a) {\n        x();\n    } else\n\n        /* c */\n        if (b) {\n            y();\n        } else\n\n            /* error */{\n            z();\n        }\n    w();\n}\n";
    let broken = "void f()\n{\n    if (a)\n    {\n        x();\n    }\n    else\n\n        /* c */\n        if (b)\n        {\n            y();\n        }\n        else\n\n            /* error */\n        {\n            z();\n        }\n    w();\n}\n";
    for (args, expected) in [
        (&[][..], attached),
        (&["--style=kr"][..], attached),
        (&["--style=allman"][..], broken),
    ] {
        check(nested, args, expected);
        check(expected, args, expected);
    }
}

// A comment row inside brackets stands at the row it precedes, as inside
// parentheses.
#[test]
fn a_comment_row_inside_brackets_stands_at_the_row_after_it() {
    let input = "static char name[\n    /* note */\n#ifdef FEATURE\n    LIMIT +\n#endif\n    1];\nvoid f()\n{\n    value = table[\n                /* note */\n                index];\n    int name[\n        /* note */\n        COUNT];\n}\n";
    check(input, &[], input);
    // A message's keyword row is no label.
    let message = "@implementation Item\n- (void)update {\n    [self apply:value\n     /* note */\n           with:other];\n}\n@end\n";
    check(
        message,
        &[],
        "@implementation Item\n- (void)update {\n    [self apply:value\n          /* note */\n          with:other];\n}\n@end\n",
    );
}

// Without a brace style a bare block's brace keeps the code after it on its
// line, whatever one-line block that code holds.
#[test]
fn a_run_in_bare_block_brace_keeps_its_code_beside_a_one_line_block() {
    let input = "void f()\n{\n    x = 1;\n    { int cnt; for(cnt=0; cnt<3; cnt++){}\n        h(cnt);\n    }\n}\n";
    let expected = "void f()\n{\n    x = 1;\n    {   int cnt;\n        for(cnt=0; cnt<3; cnt++) {}\n        h(cnt);\n    }\n}\n";
    check(input, &[], expected);
    check(expected, &[], expected);
}

// A closer pico joins onto the row before takes the space joining it out of
// the gap before its comment, whatever follows the closer, and parts a
// comment set against it.
#[test]
fn pico_joins_a_closer_taking_the_joining_space_out_of_its_comment_gap() {
    for (gap, output_gap) in [("", " "), ("  ", " "), ("     ", "    ")] {
        let input = format!(
            "int t[][2] = {{\n    {{ 1,\n      2\n    }},{gap}/* c */\n    {{ 3,\n      4\n    }}{gap}/* d */\n}};\nstruct S {{\n    int a;\n}} s, t;{gap}/* e */\n"
        );
        let expected = format!(
            "int t[][2] = {{\n    {{   1,\n        2 }},{output_gap}/* c */\n    {{   3,\n        4 }}{output_gap}/* d */\n}};\nstruct S\n{{   int a; }} s, t;{output_gap}/* e */\n"
        );
        check(&input, &["--style=pico"], &expected);
        check(&expected, &["--style=pico"], &expected);
    }
}

// A case label line stands where case labels do though a statement label
// follows it on the line.
#[test]
fn a_case_label_line_holding_a_statement_label_stands_with_the_case_labels() {
    let input = "void f()\n{\n    switch (k) {\n    case 1:\n        goto again;\n    case 2: again:\n        x();\n        break;\n    case 3: other: {\n        y();\n    }\n    }\n}\n";
    for (style, expected) in [
        (
            "--style=pico",
            "void f()\n{   switch (k)\n    {   case 1:\n            goto again;\n        case 2: again:\n            x();\n            break;\n        case 3: other:\n            {   y(); } } }\n",
        ),
        (
            "--style=lisp",
            "void f() {\n    switch (k) {\n    case 1:\n        goto again;\n    case 2: again:\n        x();\n        break;\n    case 3: other: {\n        y(); } } }\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

// A declarator after a nested body's closer stands at the member it
// declares, wherever the style sets the closer.
#[test]
fn a_declarator_after_a_nested_body_closer_stands_at_its_member() {
    let input = "struct histindex {\n\tstruct record {\n\t\tunsigned int ptr, cnt;\n\t\tstruct record *next;\n\t} **records, /* an occurrence */\n\t  **line_map; /* map */\n\tint other;\n};\n";
    for (style, expected) in [
        (
            "--style=whitesmith",
            "struct histindex\n    {\n    struct record\n        {\n        unsigned int ptr, cnt;\n        struct record *next;\n        } **records, /* an occurrence */\n    **line_map; /* map */\n    int other;\n    };\n",
        ),
        (
            "--style=ratliff",
            "struct histindex {\n    struct record {\n        unsigned int ptr, cnt;\n        struct record *next;\n        } **records, /* an occurrence */\n    **line_map; /* map */\n    int other;\n    };\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

// The brace added after an else and a comment opens at the else's own level
// in an else chain a directive split.
#[test]
fn an_else_brace_after_a_comment_in_a_directive_split_chain_opens_at_the_else() {
    let input = "int f(void)\n{\n#ifdef FEATURE\n  if(*value == 1) {\n    first();\n  }\n  else\n#endif\n    if(*value == 2) {\n      second();\n    }\n    else {\n      if(end) {\n        third();\n      }\n      else\n        /* note */\n        size = 4;\n    }\n  return 0;\n}\n";
    let expected = "int f(void)\n{\n#ifdef FEATURE\n    if(*value == 1) {\n        first();\n    } else\n#endif\n        if(*value == 2) {\n            second();\n        } else {\n            if(end) {\n                third();\n            } else\n                /* note */\n            {\n                size = 4;\n            }\n        }\n    return 0;\n}\n";
    check(input, &["--style=1tbs"], expected);
    check(expected, &["--style=1tbs"], expected);
}

// Ratliff closes a block after headers that conditional branches open
// alike at the block's own header.
#[test]
fn ratliff_closes_a_block_after_branch_alternative_openers_at_its_header() {
    let input = "void f(void) {\n  for (;;) {\n    for (;;) {\n#ifdef X\n      if (c == 1) {\n#else\n      if (c == 2) {\n#endif\n      }\n    }\n    if (e) {\n    }\n  }\n}\n";
    let expected = "void f(void) {\n    for (;;) {\n        for (;;) {\n#ifdef X\n            if (c == 1) {\n#else\n            if (c == 2) {\n#endif\n                }\n            }\n        if (e) {\n            }\n        }\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(expected, &["--style=ratliff"], expected);
}

// Ratliff closes a block at the line its header leads, though a comment
// before an else chain a directive splits leaves the engine a level off.
#[test]
fn ratliff_closes_blocks_of_a_split_else_chain_after_a_comment_at_their_headers() {
    let input = "void f(void)\n{\n#ifndef A\n  if( ready ){\n  }else\n#endif\n  {\n    if( other ){\n    }\n    /* note */\n#ifndef C\n    if( more ){\n    }else\n#endif\n    {\n      helper();\n    }\n  }\n}\n";
    let expected = "void f(void) {\n#ifndef A\n    if( ready ) {\n        }\n    else\n#endif\n        {\n        if( other ) {\n            }\n        /* note */\n#ifndef C\n        if( more ) {\n            }\n        else\n#endif\n            {\n            helper();\n            }\n        }\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(expected, &["--style=ratliff"], expected);
}

// Ratliff sets a compound literal's closer a level in after a nested one's,
// as after any row.
#[test]
fn ratliff_sets_a_compound_literal_closer_after_a_nested_one_a_level_in() {
    let input = "void f(void) {\n  for (;;) {\n    (*ci)[i++] = (uv_cpu_info_t) {\n      .model = p,\n      .cpu_times = (struct times) {\n        .user = 10,\n        .irq  = 10,\n      },\n    };\n  }\n  x = (struct T) {\n    .a = 1,\n    .b = 2,\n  };\n}\n";
    let expected = "void f(void) {\n    for (;;) {\n        (*ci)[i++] = (uv_cpu_info_t) {\n            .model = p,\n            .cpu_times = (struct times) {\n                .user = 10,\n                .irq  = 10,\n                },\n            };\n        }\n    x = (struct T) {\n        .a = 1,\n        .b = 2,\n        };\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(expected, &["--style=ratliff"], expected);
}

// A bare block's brace after a comment stays where the source set it: only a
// header's brace moves before a comment trailing the header.
#[test]
fn a_bare_block_brace_after_a_comment_stays_after_it() {
    let input = "void f()\n{\n    if (a)\n        /* note */{\n        y();\n    }\n    x = 1; /* c */ {\n        y();\n    }\n    x = 1; /* c */{\n        y();\n    }\n}\n";
    for args in [&[][..], &["--style=kr"][..]] {
        check(input, args, input);
    }
    check(
        "void f()\n{\n    if (a) /* c */ {\n        y();\n    }\n}\n",
        &[],
        "void f()\n{\n    if (a) { /* c */\n        y();\n    }\n}\n",
    );
}

// A header broken off the statement before it on its source line keeps the
// gap before its comment, as a statement broken off another does.
#[test]
fn a_header_broken_off_a_statement_keeps_its_comment_gap() {
    let input = "void f()\n{\n    first(); second(); // note\n    if (a)\n        x(); else // note\n    value = 1; while (a) // note\n        x();\n}\n";
    let expected = "void f()\n{\n    first();\n    second(); // note\n    if (a)\n        x();\n    else // note\n        value = 1;\n    while (a) // note\n        x();\n}\n";
    for args in [&[][..], &["--style=kr"][..], &["--style=allman"][..]] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn adding_braces_wraps_a_statement_whose_string_ends_in_an_escaped_quote_after_a_backslash() {
    let input = "void f()\n{\n    if (c == 1)\n        add(out, \"\\\\\\\"\");\n    else\n        add(out, \"x\");\n}\n";
    let expected = "void f()\n{\n    if (c == 1) {\n        add(out, \"\\\\\\\"\");\n    }\n    else {\n        add(out, \"x\");\n    }\n}\n";
    check(input, &["--add-braces"], expected);
    check(expected, &["--add-braces"], expected);
}

// Pico keeps a bare one-line block whole as it keeps a header's, the way
// styles keeping one-line blocks do.
#[test]
fn pico_keeps_a_bare_one_line_block_as_it_keeps_a_headers() {
    let input = "void f()\n{\n    value = 1;\n    { value = 0; }\n    if (ready) { value = 2; }\n    { value = 3; } more();\n}\n";
    let expected = "void f()\n{   value = 1;\n    { value = 0; }\n    if (ready) { value = 2; }\n    { value = 3; } more(); }\n";
    check(input, &["--style=pico"], expected);
    check(expected, &["--style=pico"], expected);
}

#[test]
fn an_element_opening_a_preprocessor_branch_after_one_without_a_comma_stands_with_the_rows() {
    let file_scope =
        "struct Item items[] = {\n#if A\n    {1, 2}\n#else\n    {3, 4},\n    {5, 6}\n#endif\n};\n";
    let block = "void f()\n{\n    static const struct Item items[] = {\n#if A\n        {1, 2}\n#else\n        {5, 6},\n        {7, 8}\n#endif\n    };\n}\n";
    for (style, input, expected) in [
        ("vtk", file_scope, file_scope),
        (
            "whitesmith",
            file_scope,
            "struct Item items[] = {\n#if A\n        {1, 2}\n#else\n        {3, 4},\n        {5, 6}\n#endif\n    };\n",
        ),
        (
            "ratliff",
            file_scope,
            "struct Item items[] = {\n#if A\n        {1, 2}\n#else\n        {3, 4},\n        {5, 6}\n#endif\n    };\n",
        ),
        (
            "vtk",
            block,
            "void f()\n{\n    static const struct Item items[] =\n        {\n#if A\n            {1, 2}\n#else\n            {5, 6},\n            {7, 8}\n#endif\n        };\n}\n",
        ),
        (
            "whitesmith",
            block,
            "void f()\n    {\n    static const struct Item items[] =\n        {\n#if A\n            {1, 2}\n#else\n            {5, 6},\n            {7, 8}\n#endif\n        };\n    }\n",
        ),
        (
            "ratliff",
            block,
            "void f() {\n    static const struct Item items[] = {\n#if A\n            {1, 2}\n#else\n            {5, 6},\n            {7, 8}\n#endif\n        };\n    }\n",
        ),
    ] {
        let style = format!("--style={style}");
        check(input, &[&style], expected);
        check(expected, &[&style], expected);
    }
}

#[test]
fn vtk_sets_one_line_element_rows_at_the_rows_and_broken_element_braces_past_them() {
    // Within a block the rows stand at the indented brace, as the broken
    // elements' braces do.
    let input = "void f()\n{\n    static const struct Item items[] = {\n        { name(\"a\"), NULL, first,\n          0, 0 },\n        { name(\"b\"), NULL, second, 0, 0 },\n        { name(\"c\"), NULL,\n          third, 0, 0 },\n    };\n}\n";
    let expected = "void f()\n{\n    static const struct Item items[] =\n        {\n            {\n            name(\"a\"), NULL, first,\n            0, 0\n            },\n            { name(\"b\"), NULL, second, 0, 0 },\n            {\n            name(\"c\"), NULL,\n            third, 0, 0\n            },\n        };\n}\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
    for (input, expected) in [
        (
            "static Item items[] = {\n    { name(\"a\"), NULL, first,\n      0, 0 },\n\n    { name(\"b\"), NULL, second, 0, 0 },\n\n    { name(\"c\"), NULL, third, 0, 0 },\n    { name(\"d\"), NULL,\n      fourth, 0, 0 },\n    { name(\"e\"), NULL, fifth, 0, 0 },\n};\n",
            "static Item items[] = {\n        {\n        name(\"a\"), NULL, first,\n        0, 0\n        },\n\n    { name(\"b\"), NULL, second, 0, 0 },\n\n    { name(\"c\"), NULL, third, 0, 0 },\n        {\n        name(\"d\"), NULL,\n        fourth, 0, 0\n        },\n    { name(\"e\"), NULL, fifth, 0, 0 },\n};\n",
        ),
        (
            "static Item items[] = {\n    { name(\"b\"), NULL, second, 0, 0 },\n    { name(\"d\"), NULL,\n      fourth, 0, 0 },\n    { name(\"e\"), NULL, fifth, 0, 0 },\n};\n",
            "static Item items[] = {\n    { name(\"b\"), NULL, second, 0, 0 },\n        {\n        name(\"d\"), NULL,\n        fourth, 0, 0\n        },\n    { name(\"e\"), NULL, fifth, 0, 0 },\n};\n",
        ),
    ] {
        check(input, &["--style=vtk"], expected);
        check(expected, &["--style=vtk"], expected);
    }
}

#[test]
fn a_label_block_after_a_bare_block_closes_where_label_blocks_do() {
    let input = "void f()\n{\n    { value=0; }\n    done: {\n        value=0;\n    }\n}\n";
    let expected = "void f() {\n        {\n        value=0;\n        }\ndone: {\n        value=0;\n        }\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(expected, &["--style=ratliff"], expected);
}

#[test]
fn breaking_one_line_headers_breaks_an_else_header_once_added_braces_take_the_statement_off() {
    let input = "void f()\n{\n    if (ready) { helper(); } else switch (state) {\n    case 1:\n        first();\n    }\n    if (ready) helper(); else for (;;) {\n        first();\n    }\n    if (ready) helper(); else while (x) {\n        first();\n    }\n}\n";
    let expected = "void f() {\n    if (ready) {\n        helper(); }\n    else\n        switch (state) {\n        case 1:\n            first(); }\n    if (ready) {\n        helper(); }\n    else\n        for (;;) {\n            first(); }\n    if (ready) {\n        helper(); }\n    else\n        while (x) {\n            first(); } }\n";
    let args = ["--style=lisp", "--add-braces", "--break-one-line-headers"];
    check(input, &args, expected);
    check(expected, &args, expected);
}

#[test]
fn an_else_after_an_added_one_line_block_starts_its_line_as_after_a_written_one() {
    let input = "void f()\n{\n    if (ready) helper(); else other();\n    if (ready) helper(); else do {\n        first();\n    } while (state);\n    if (ready) helper();\n    else\n        other();\n}\n";
    for (args, expected) in [
        (
            &["--style=kr", "--add-one-line-braces"][..],
            "void f()\n{\n    if (ready) { helper(); }\n    else { other(); }\n    if (ready) { helper(); }\n    else do {\n            first();\n        } while (state);\n    if (ready) { helper(); }\n    else\n    { other(); }\n}\n",
        ),
        (
            &["--style=java", "--add-one-line-braces", "--break-blocks"][..],
            "void f() {\n    if (ready) { helper(); }\n    else { other(); }\n\n    if (ready) { helper(); }\n    else do {\n            first();\n        } while (state);\n\n    if (ready) { helper(); }\n    else\n    { other(); }\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn added_braces_close_a_body_its_else_follows_on_the_line_before_the_else() {
    let input = "void f()\n{\n    if (ready)\n        helper(); else for (;;) {\n        first();\n    }\n    if (ready)\n        helper(); else other();\n    if (ready)\n        helper(); else while (more) next();\n}\n";
    for (args, expected) in [
        (
            &["--style=1tbs"][..],
            "void f()\n{\n    if (ready) {\n        helper();\n    } else for (;;) {\n            first();\n        }\n    if (ready) {\n        helper();\n    } else {\n        other();\n    }\n    if (ready) {\n        helper();\n    } else while (more) {\n            next();\n        }\n}\n",
        ),
        (
            &["--style=allman", "--add-braces"][..],
            "void f()\n{\n    if (ready)\n    {\n        helper();\n    }\n    else for (;;)\n        {\n            first();\n        }\n    if (ready)\n    {\n        helper();\n    }\n    else\n    {\n        other();\n    }\n    if (ready)\n    {\n        helper();\n    }\n    else while (more)\n        {\n            next();\n        }\n}\n",
        ),
        (
            &["--style=lisp", "--add-braces"][..],
            "void f() {\n    if (ready) {\n        helper(); }\n    else for (;;) {\n            first(); }\n    if (ready) {\n        helper(); }\n    else {\n        other(); }\n    if (ready) {\n        helper(); }\n    else while (more) {\n            next(); } }\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn added_braces_wrap_a_statement_holding_a_block_comment() {
    let input = "void f(void)\n{\n    if (ready)\n        helper(opt, /* detailed */ 0, result);\n    if (ready) helper(opt, /* detailed */ 0);\n    if (r != 0)\n        r = g(file, 85 /* F_X */);  /* tail */\n    if (n > m) return -1 /* bad */;\n    if (z) *oz++ = 0 /* back */;\n}\n";
    for (args, expected) in [
        (
            &["--style=1tbs"][..],
            "void f(void)\n{\n    if (ready) {\n        helper(opt, /* detailed */ 0, result);\n    }\n    if (ready) {\n        helper(opt, /* detailed */ 0);\n    }\n    if (r != 0) {\n        r = g(file, 85 /* F_X */);    /* tail */\n    }\n    if (n > m) {\n        return -1 /* bad */;\n    }\n    if (z) {\n        *oz++ = 0 /* back */;\n    }\n}\n",
        ),
        (
            &["--add-one-line-braces"][..],
            "void f(void)\n{\n    if (ready)\n    { helper(opt, /* detailed */ 0, result); }\n    if (ready) { helper(opt, /* detailed */ 0); }\n    if (r != 0)\n    { r = g(file, 85 /* F_X */); }  /* tail */\n    if (n > m) { return -1 /* bad */; }\n    if (z) { *oz++ = 0 /* back */; }\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn a_comment_after_an_added_run_in_block_keeps_its_column() {
    let input = "void f(void)\n{\n    if (r != 0)\n        r = g(file, 85);  /* tail */\n    if (r) x = 1;  /* t2 */\n}\n";
    let expected = "void f(void)\n{   if (r != 0)\n    {   r = g(file, 85); }  /* tail */\n    if (r) { x = 1; }  /* t2 */\n}\n";
    check(input, &["--style=pico", "--add-one-line-braces"], expected);
    check(
        expected,
        &["--style=pico", "--add-one-line-braces"],
        expected,
    );
}

#[test]
fn whitesmith_braces_after_a_directive_stand_where_their_owner_puts_them() {
    for group in ["#ifdef A\n#endif\n", "#if A\n#else\n#endif\n"] {
        let input = format!("void helper()\n{{\n{group}    {{\n        value++;\n    }}\n}}\n");
        let expected = format!(
            "void helper()\n    {{\n{group}        {{\n        value++;\n        }}\n    }}\n"
        );
        check(&input, &["--style=whitesmith"], &expected);
        check(&expected, &["--style=whitesmith"], &expected);
    }
    for (input, expected) in [
        (
            "struct Item\n#ifdef A\n    : Base\n#endif\n{\n    int value;\n};\n",
            "struct Item\n#ifdef A\n    : Base\n#endif\n    {\n    int value;\n    };\n",
        ),
        (
            "class Item\n{\npublic:\n    void helper()\n#ifdef A\n#endif\n    {\n        value++;\n    }\n};\n",
            "class Item\n    {\n    public:\n        void helper()\n#ifdef A\n#endif\n            {\n            value++;\n            }\n    };\n",
        ),
        (
            "namespace demo\n#ifdef A\n#endif\n{\nint value;\n}\n",
            "namespace demo\n#ifdef A\n#endif\n{\nint value;\n}\n",
        ),
        (
            "namespace demo\n#define X\n{\nint value;\n}\n",
            "namespace demo\n#define X\n{\nint value;\n}\n",
        ),
    ] {
        check(input, &["--style=whitesmith"], expected);
        check(expected, &["--style=whitesmith"], expected);
    }
}

#[test]
fn a_directive_comment_before_a_block_is_no_operator_for_the_closers_after_it() {
    for directive in ["#else /* first */\n", "#endif /* first */\n"] {
        let tail = if directive.starts_with("#else") {
            "#endif\n"
        } else {
            ""
        };
        let input = format!(
            "#ifdef A\n#else\nvoid helper(int value)\n#endif\n{{\n#ifdef B\n{directive}    {{\n    }}\n{tail}}}\n"
        );
        let expected = format!(
            "#ifdef A\n#else\nvoid helper(int value)\n#endif\n    {{\n#ifdef B\n{directive}        {{\n        }}\n{tail}    }}\n"
        );
        check(&input, &["--style=whitesmith"], &expected);
        check(&expected, &["--style=whitesmith"], &expected);
    }
}

#[test]
fn ratliff_closes_a_block_after_alike_branches_at_the_header_it_shares_a_line_with() {
    let input = "void helper(int value)\n{\n    if (value != 0) do {\n#ifdef A\n        value--;\n#else\n        value -= 2;\n#endif\n    } while (value > 0);\n}\n";
    let expected = "void helper(int value) {\n    if (value != 0) do {\n#ifdef A\n            value--;\n#else\n            value -= 2;\n#endif\n            }\n        while (value > 0);\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(expected, &["--style=ratliff"], expected);
}

#[test]
fn ratliff_closes_a_block_at_the_opener_of_the_last_branch_that_opens_it() {
    let input = "void helper(Item *item)\n{\n#ifndef A\n  assert( item!=0 );\n  {\n#else\n  if( item ){\n#endif\n    item->value = 0;\n  }\n}\n";
    let expected = "void helper(Item *item) {\n#ifndef A\n    assert( item!=0 );\n        {\n#else\n    if( item ) {\n#endif\n        item->value = 0;\n        }\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(expected, &["--style=ratliff"], expected);
}

#[test]
fn indented_brace_styles_close_an_element_list_nested_in_a_compound_literal_at_its_elements() {
    let shallow = "void helper()\n{\n    value = (struct Item) {\n        .first = 1,\n        .inner = {\n            .second = 2,\n        },\n    };\n}\n";
    let deep = "void helper()\n{\n    value = (struct Item) {\n        .first = 1,\n        .inner = {\n            .second = { 2, 3 },\n            .third = {\n                4,\n            },\n        },\n    };\n}\n";
    for (input, style, expected) in [
        (
            shallow,
            "--style=whitesmith",
            "void helper()\n    {\n    value = (struct Item)\n        {\n        .first = 1,\n        .inner = {\n            .second = 2,\n            },\n        };\n    }\n",
        ),
        (
            shallow,
            "--style=vtk",
            "void helper()\n{\n    value = (struct Item)\n        {\n        .first = 1,\n        .inner = {\n            .second = 2,\n            },\n        };\n}\n",
        ),
        (
            shallow,
            "--style=ratliff",
            "void helper() {\n    value = (struct Item) {\n        .first = 1,\n        .inner = {\n            .second = 2,\n            },\n        };\n    }\n",
        ),
        (
            deep,
            "--style=whitesmith",
            "void helper()\n    {\n    value = (struct Item)\n        {\n        .first = 1,\n        .inner = {\n            .second = { 2, 3 },\n            .third = {\n                4,\n                },\n            },\n        };\n    }\n",
        ),
        (
            deep,
            "--style=vtk",
            "void helper()\n{\n    value = (struct Item)\n        {\n        .first = 1,\n        .inner = {\n            .second = { 2, 3 },\n            .third = {\n                4,\n                },\n            },\n        };\n}\n",
        ),
        (
            deep,
            "--style=ratliff",
            "void helper() {\n    value = (struct Item) {\n        .first = 1,\n        .inner = {\n            .second = { 2, 3 },\n            .third = {\n                4,\n                },\n            },\n        };\n    }\n",
        ),
    ] {
        check(input, &[style], expected);
        check(expected, &[style], expected);
    }
}

#[test]
fn ratliff_sets_a_compound_literal_closer_its_line_shares_with_a_nested_one_a_level_in() {
    let input = "void helper(void *arg)\n{\n    value = (struct Item) {\n        .u.obj = {\n            .first = 1,\n        } };\n}\n";
    let expected = "void helper(void *arg) {\n    value = (struct Item) {\n        .u.obj = {\n            .first = 1,\n            }\n        };\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(expected, &["--style=ratliff"], expected);
}

#[test]
fn a_one_line_block_after_a_comment_leading_its_line_leaves_no_level_to_the_next_header() {
    for body in ["/* note */ helper();", "/* note */ { helper(); }"] {
        let input = format!(
            "void f()\n{{\n    if (ready)\n        {body}\n    while (ready)\n        /* note */\n        helper();\n}}\n"
        );
        let expected = "void f()\n{\n    if (ready)\n        /* note */ { helper(); }\n    while (ready)\n        /* note */\n    { helper(); }\n}\n";
        check(&input, &["--add-one-line-braces"], expected);
        check(expected, &["--add-one-line-braces"], expected);
    }
    let input = "void f()\n{\n    if (ready)\n        helper();\n    else\n        /* note */ other();\n    while (ready)\n        /* note */\n        helper();\n}\n";
    let expected = "void f()\n{\n    if (ready)\n    { helper(); }\n    else\n        /* note */ { other(); }\n    while (ready)\n        /* note */\n    { helper(); }\n}\n";
    check(input, &["--add-one-line-braces"], expected);
    check(expected, &["--add-one-line-braces"], expected);
}

#[test]
fn tab_indents_fill_an_element_brace_run_in_to_rows_below_with_a_tab() {
    let input = "void f()\n{\n    static const struct Item items[] = {\n        {  1,  2 },\n        {5,\n         6}\n    };\n}\n";
    let expected = "void f()\n{\n\tstatic const struct Item items[] = {\n\t\t{  1,  2 },\n\t\t{\t5,\n\t\t\t6\n\t\t}\n\t};\n}\n";
    for indent in ["--indent=tab=4", "--indent=force-tab=4"] {
        check(input, &[indent], expected);
        check(expected, &[indent], expected);
    }
}

#[test]
fn vtk_sets_a_branch_row_after_a_broken_element_at_the_aggregate_rows() {
    let input = "static const struct Item items[] = {\n#if A\n    {1, 2}\n#elif B\n    {1, 2},\n    {3,\n     4}\n#else\n    {5, 6},\n    {7, 8}\n#endif\n};\n";
    let expected = "static const struct Item items[] = {\n#if A\n    {1, 2}\n#elif B\n    {1, 2},\n    {\n        3,\n        4\n    }\n#else\n    {5, 6},\n    {7, 8}\n#endif\n};\n";
    check(input, &["--style=vtk"], expected);
    check(expected, &["--style=vtk"], expected);
}

#[test]
fn a_header_after_a_statement_its_line_ends_nests_in_no_header_before_it() {
    let input = "void f()\n{\n    if (ready) helper(); else if (x) {\n        first();\n    }\n    if (ready) helper(); else while (x) {\n        first();\n    }\n    for (;;) if (x) {\n        first();\n    }\n}\n";
    let expected = "void f() {\n    if (ready) helper(); else if (x) {\n        first(); }\n    if (ready) helper(); else while (x) {\n            first(); }\n    for (;;) if (x) {\n            first(); } }\n";
    check(input, &["--style=lisp"], expected);
    check(expected, &["--style=lisp"], expected);
}

// Breaking blocks pads around a statement, never inside it: a header an
// `else` kept on a statement line heads runs straight into its block, as
// it does on a line of its own.
#[test]
fn break_blocks_pads_no_line_between_a_header_after_a_kept_else_and_its_block() {
    let input = "void f()\n{\n    if (ready) helper(); else do {\n        first();\n    } while (state);\n    if (ready)\n        helper(); else for (;;) {\n        first();\n    }\n}\n";
    for (args, expected) in [
        (
            &["--style=pico", "--break-blocks"][..],
            "void f()\n{   if (ready) helper(); else do\n        {   first(); }\n        while (state);\n\n    if (ready)\n        helper(); else for (;;)\n        {   first(); } }\n",
        ),
        (
            &["--style=lisp", "--break-blocks=all"][..],
            "void f() {\n    if (ready) helper(); else do {\n            first(); }\n        while (state);\n\n    if (ready)\n        helper(); else for (;;) {\n            first(); } }\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn break_blocks_pads_after_a_kept_one_line_block_that_continues_a_header_block() {
    let input = "void f()\n{\n    if (state) {\n        a();\n    }\n    { value = 0; }\n    value = 1;\n    if (state) {\n        a();\n    }\n    {\n        value = 0;\n    }\n    value = 1;\n}\n";
    let expected = "void f()\n{\n    if (state) {\n        a();\n    }\n\n    { value = 0; }\n\n    value = 1;\n\n    if (state) {\n        a();\n    }\n\n    {\n        value = 0;\n    }\n\n    value = 1;\n}\n";
    let args = [
        "--style=stroustrup",
        "--keep-one-line-blocks",
        "--break-blocks",
    ];
    check(input, &args, expected);
    check(expected, &args, expected);
}

#[test]
fn a_braceless_body_after_a_label_heading_its_header_stands_past_the_header() {
    let input = "void f()\n{\n    switch (kind) {\n    default: {\n        break;\n    }\n    }\n    done: if (ready) { value=0; }\n}\n";
    let expected = "void f() {\n    switch (kind) {\n    default: {\n        break; } }\ndone: if (ready)\n        value=0;  }\n";
    check(input, &["--style=lisp", "--remove-braces"], expected);
    check(expected, &["--style=lisp", "--remove-braces"], expected);
}

#[test]
fn a_braceless_body_of_a_header_an_else_keeps_on_a_statement_line_nests_from_its_if() {
    let input = "void f()\n{\n    if (ready) helper(); else for (;;)\n        first();\n    if (ready)\n        helper(); else for (;;)\n        first();\n    if (ready)\n        helper(); else while (x) for (;;)\n        first();\n    if (ready)\n        helper(); else if (x)\n        first();\n}\n";
    let expected = "void f()\n{\n    if (ready) helper(); else for (;;)\n            first();\n    if (ready)\n        helper(); else for (;;)\n            first();\n    if (ready)\n        helper(); else while (x) for (;;)\n                first();\n    if (ready)\n        helper(); else if (x)\n        first();\n}\n";
    check(input, &["--style=allman", "-o"], expected);
    check(expected, &["--style=allman", "-o"], expected);
}

#[test]
fn break_blocks_pads_after_a_one_line_block_on_a_line_a_comment_leads() {
    let input = "void f()\n{\n    if (ready)\n        /* note */ helper();\n    else\n        /* note */ other();\n    while (ready)\n        /* note */\n        helper();\n}\n";
    let expected = "void f()\n{   if (ready)\n        /* note */ { helper(); }\n\n    else\n        /* note */ { other(); }\n\n    while (ready)\n        /* note */\n    { helper(); }\n}\n";
    let args = [
        "--style=horstmann",
        "--break-blocks=all",
        "--add-one-line-braces",
    ];
    check(input, &args, expected);
    check(expected, &args, expected);
    let input = "void f()\n{\n    if (ready)\n        helper();\n    else\n        /* note */ other();\n    value = 1;\n}\n";
    let expected = "void f()\n{\n    if (ready)\n    { helper(); }\n    else\n        /* note */ { other(); }\n\n    value = 1;\n}\n";
    let args = ["--style=kr", "--break-blocks", "--add-one-line-braces"];
    check(input, &args, expected);
    check(expected, &args, expected);
}

#[test]
fn kept_one_line_blocks_take_braces_added_after_a_comment_leading_the_statement_line() {
    let input = "void f()\n{\n    if (ready)\n        /* note */ helper();\n    else\n        /* note */ other(); // tail\n}\n";
    for (args, expected) in [
        (
            &["--style=kr", "--add-braces", "--keep-one-line-blocks"][..],
            "void f()\n{\n    if (ready)\n        /* note */ { helper(); }\n    else\n        /* note */ { other(); } // tail\n}\n",
        ),
        (
            &["--style=allman", "--add-braces", "--keep-one-line-blocks"][..],
            "void f()\n{\n    if (ready)\n        /* note */ { helper(); }\n    else\n        /* note */ { other(); } // tail\n}\n",
        ),
        (
            &["--style=pico", "--add-braces", "--keep-one-line-blocks"][..],
            "void f()\n{   if (ready)\n        /* note */ { helper(); }\n    else\n        /* note */ { other(); } // tail\n}\n",
        ),
        // Breaking one-line headers breaks the block it adds there.
        (
            &[
                "--style=1tbs",
                "--keep-one-line-blocks",
                "--break-one-line-headers",
            ][..],
            "void f()\n{\n    if (ready)\n        /* note */ {\n        helper();\n    } else\n        /* note */ {\n        other();    // tail\n    }\n}\n",
        ),
    ] {
        check(input, args, expected);
        check(expected, args, expected);
    }
}

#[test]
fn a_member_split_off_its_return_type_after_an_indented_access_label_stands_at_the_members() {
    let input =
        "class Item {\npublic:\n    int value() const\n    {\n        return value_;\n    }\n};\n";
    let expected = "class Item {\n  public:\n    int\n    value() const {\n        return value_;\n    }\n};\n";
    let args = [
        "--style=google",
        "--break-return-type",
        "--keep-one-line-statements",
    ];
    check(input, &args, expected);
    check(expected, &args, expected);
}

#[test]
fn tab_indents_set_an_indented_directive_among_arguments_as_the_argument_rows() {
    let input = "void f()\n{\n    result = helper(first,\n#ifdef FEATURE\n                    second,\n#endif\n                    third);\n}\n";
    for (indent, expected) in [
        (
            "--indent=tab=4",
            "void f()\n{\n\tresult = helper(first,\n\t                #ifdef FEATURE\n\t                second,\n\t                #endif\n\t                third);\n}\n",
        ),
        (
            "--indent=force-tab=4",
            "void f()\n{\n\tresult = helper(first,\n\t\t\t\t\t#ifdef FEATURE\n\t\t\t\t\tsecond,\n\t\t\t\t\t#endif\n\t\t\t\t\tthird);\n}\n",
        ),
    ] {
        check(input, &["--indent-preproc-cond", indent], expected);
        check(expected, &["--indent-preproc-cond", indent], expected);
    }
}

#[test]
fn horstmann_runs_an_access_label_in_at_its_column_whatever_the_gap() {
    let input = "class Widget : public Base\n{\npublic:\n    int a;\nprivate:\n    int b;\n};\n";
    for (indent, expected) in [
        (
            "--indent=spaces=2",
            "class Widget : public Base\n{public:\n  int a;\n private:\n  int b;\n};\n",
        ),
        (
            "--indent=tab=2",
            "class Widget : public Base\n{public:\n\tint a;\n private:\n\tint b;\n};\n",
        ),
        (
            "--indent=force-tab=2",
            "class Widget : public Base\n{public:\n\tint a;\n private:\n\tint b;\n};\n",
        ),
    ] {
        let args = ["--style=horstmann", "--indent-modifiers", indent];
        check(input, &args, expected);
        check(expected, &args, expected);
    }
}

#[test]
fn indented_access_labels_leave_the_members_at_the_class_level_with_an_odd_indent() {
    let input =
        "class Widget\n{\npublic:\n    int a;\n    void run()\n    {\n        a = 1;\n    }\n};\n";
    for (indent, expected) in [
        (
            "--indent=spaces=3",
            "class Widget\n{\n public:\n   int a;\n   void run()\n   {\n      a = 1;\n   }\n};\n",
        ),
        (
            "--indent=spaces=5",
            "class Widget\n{\n  public:\n     int a;\n     void run()\n     {\n          a = 1;\n     }\n};\n",
        ),
    ] {
        let args = ["--style=allman", "--indent-modifiers", indent];
        check(input, &args, expected);
        check(expected, &args, expected);
    }
}

#[test]
fn an_attached_return_type_ending_in_a_pointer_keeps_the_pointer_alignment() {
    let input = "static struct Item *\nmake(int value);\nstatic struct Item &\nfind(int value);\n";
    for (align, expected) in [
        (
            "--align-pointer=type",
            "static struct Item* make(int value);\nstatic struct Item& find(int value);\n",
        ),
        (
            "--align-pointer=middle",
            "static struct Item * make(int value);\nstatic struct Item & find(int value);\n",
        ),
        (
            "--align-pointer=name",
            "static struct Item *make(int value);\nstatic struct Item &find(int value);\n",
        ),
    ] {
        let args = [align, "--attach-return-type-decl"];
        check(input, &args, expected);
        check(expected, &args, expected);
    }
}

#[test]
fn break_blocks_pads_around_a_header_a_comment_leads_on_its_line() {
    let input =
        "void f()\n{\n    value = 1;\n    /* c */ if (x)\n        a();\n    value = 2;\n}\n";
    let expected =
        "void f()\n{\n    value = 1;\n\n    /* c */ if (x)\n        a();\n\n    value = 2;\n}\n";
    check(input, &["--style=linux", "--break-blocks"], expected);
    check(expected, &["--style=linux", "--break-blocks"], expected);
}

#[test]
fn breaking_one_line_headers_leaves_a_comment_led_body_line_of_a_header_directives_split() {
    let input = "void f()\n{\n#ifdef A\n    if (q > 9)\n#else\n    if (q > 15)\n#endif\n        /*d*/ Bug(\"x\");\n}\n";
    check(input, &["--style=kr", "--break-one-line-headers"], input);
}

#[test]
fn adding_braces_leaves_no_body_indent_after_a_comment_led_body_of_a_header_directives_split() {
    let input = "void f()\n{\n#ifdef D\n#ifdef A\n    if (q > 9)\n#else\n    if (q > 15)\n#endif\n        /*d*/ Bug(\"x\");\n#endif\n    if (q) {\n        value = 1;\n    }\n}\n";
    check(input, &["--style=kr", "--add-braces"], input);
}

#[test]
fn indented_conditionals_close_a_case_block_group_at_the_column_of_its_opening_directive() {
    let input = "void f()\n{\nswitch (state) {\ncase 2:\n#ifdef TRACE\n{\n    push(state);\n}\n#endif\nbreak;\n}\n}\n";
    let expected = "void f()\n    {\n    switch (state)\n        {\n        case 2:\n            #ifdef TRACE\n            {\n            push(state);\n            }\n            #endif\n        break;\n        }\n    }\n";
    check(
        input,
        &["--style=whitesmith", "--indent-preproc-cond"],
        expected,
    );
}

#[test]
fn a_comment_between_declarators_stands_at_the_declarator_after_an_indented_initializer_closer() {
    let input = "static const Item\n    first = {\n        1,\n        2\n    },\n    /* note */\n    second = {\n        3,\n        4\n    };\n";
    check(
        input,
        &["--style=ratliff"],
        "static const Item\nfirst = {\n    1,\n    2\n    },\n/* note */\nsecond = {\n    3,\n    4\n    };\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "static const Item\nfirst =\n    {\n    1,\n    2\n    },\n/* note */\nsecond =\n    {\n    3,\n    4\n    };\n",
    );
}

#[test]
fn whitesmith_sets_the_brace_of_a_declarator_an_attribute_leads_a_level_past_its_declaration() {
    let input = "static const Item\n    first = {\n        1,\n        2\n    },\n    __attribute__ ((deprecated))\n    second = {\n        3,\n        4\n    };\n";
    check(
        input,
        &["--style=whitesmith"],
        "static const Item\nfirst =\n    {\n    1,\n    2\n    },\n__attribute__ ((deprecated))\nsecond =\n    {\n    3,\n    4\n    };\n",
    );
}

#[test]
fn breaking_one_line_headers_parts_a_comment_leading_a_body_line_from_its_statement() {
    let input = "void f() {\n    if (ready)\n        /* note */ helper();\n    else\n        /* note */ other();\n    x = 1;\n}\n";
    check(
        input,
        &["--style=kr", "--break-one-line-headers"],
        "void f()\n{\n    if (ready)\n        /* note */\n        helper();\n    else\n        /* note */\n        other();\n    x = 1;\n}\n",
    );
}

#[test]
fn an_else_body_a_comment_leads_after_a_conditional_stays_on_its_line() {
    let input = "void f()\n{\n    if (ready)\n        helper();\n    else\n#ifdef A\n        /* note */ other();\n#else\n        /* note */ third();\n#endif\n    x = 1;\n}\n";
    check(input, &["--style=kr"], input);
    check(input, &["--style=kr", "--break-one-line-headers"], input);
}

#[test]
fn a_header_and_body_comments_lead_close_its_body_before_a_closing_directive() {
    let input = "void f()\n{\n#ifdef DEBUG\n    /*d*/ if (b > n)\n        /*d*/ Bug(\"x\");\n#endif\n    if (b < n)\n        return 0;\n    x = 1;\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n#ifdef DEBUG\n    /*d*/ if (b > n)\n        /*d*/ Bug(\"x\");\n#endif\n    if (b < n)\n        return 0;\n    x = 1;\n    }\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n#ifdef DEBUG\n    /*d*/ if (b > n)\n        /*d*/ Bug(\"x\");\n#endif\n    if (b < n)\n        return 0;\n    x = 1;\n    }\n",
    );
}

#[test]
fn a_case_block_holding_case_labels_closes_under_its_brace_and_keeps_its_comments_in_its_body() {
    let input = "void run(int op)\n{\n    switch (op) {\n    case REOPEN: {\n        if (cursor) {\n            reset(cursor);\n            goto hints;\n        }\n        /* fall through into open */\n    case OPEN_READ:\n    case OPEN_WRITE:\n        open(cursor);\nhints:\n        apply(cursor);\n        break;\n    }\n    case CLOSE:\n        close(cursor);\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void run(int op)\n{\n    switch (op) {\n    case REOPEN: {\n        if (cursor) {\n            reset(cursor);\n            goto hints;\n        }\n        /* fall through into open */\n        case OPEN_READ:\n        case OPEN_WRITE:\n            open(cursor);\nhints:\n            apply(cursor);\n            break;\n    }\n    case CLOSE:\n        close(cursor);\n        break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=allman"],
        "void run(int op)\n{\n    switch (op)\n    {\n    case REOPEN:\n    {\n        if (cursor)\n        {\n            reset(cursor);\n            goto hints;\n        }\n        /* fall through into open */\n        case OPEN_READ:\n        case OPEN_WRITE:\n            open(cursor);\nhints:\n            apply(cursor);\n            break;\n    }\n    case CLOSE:\n        close(cursor);\n        break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void run(int op)\n    {\n    switch (op)\n        {\n        case REOPEN:\n            {\n            if (cursor)\n                {\n                reset(cursor);\n                goto hints;\n                }\n            /* fall through into open */\n            case OPEN_READ:\n            case OPEN_WRITE:\n                open(cursor);\nhints:\n                apply(cursor);\n                break;\n            }\n        case CLOSE:\n            close(cursor);\n            break;\n        }\n    }\n",
    );
}

#[test]
fn a_comment_after_an_element_brace_stays_on_the_brace_line_in_every_style() {
    let input = "static const struct Item items[] = {\n  {\"first\",\n   \"one\", 0, 1},\n  { /* second item */\n    \"second\",\n    \"two\", 0, 2 },\n};\n";
    check(
        input,
        &["--mode=c"],
        "static const struct Item items[] = {\n    {   \"first\",\n        \"one\", 0, 1\n    },\n    { /* second item */\n        \"second\",\n        \"two\", 0, 2\n    },\n};\n",
    );
    let broken = "static const struct Item items[] = {\n    {\n        \"first\",\n        \"one\", 0, 1\n    },\n    { /* second item */\n        \"second\",\n        \"two\", 0, 2\n    },\n};\n";
    check(input, &["--style=java"], broken);
    check(input, &["--style=kr"], broken);
}

#[test]
fn any_unconditional_directive_between_a_case_label_and_a_brace_makes_the_brace_a_plain_block() {
    let input = "void f()\n{\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n    {\n        pop(state);\n    }\n    break;\n    case 2:\n#pragma once\n    {\n        push(state);\n    }\n    break;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n        {\n            pop(state);\n        }\n        break;\n    case 2:\n#pragma once\n        {\n            push(state);\n        }\n        break;\n    }\n}\n",
    );
}

#[test]
fn lisp_padding_narrows_the_gap_before_a_comment_after_added_braces() {
    let input = "void f()\n{\n    if (ready) value=1;   // note\n}\n";
    check(
        input,
        &["--style=lisp", "--add-braces", "--pad-oper"],
        "void f() {\n    if (ready) {\n        value = 1; } // note\n}\n",
    );
}

#[test]
fn padding_the_row_a_closer_joins_narrows_the_gap_before_its_comment() {
    let lone = "void f()\n{\n    if (ready) {\n        value=a+b+c+d;\n    }        // note\n}\n";
    let nested = "void f()\n{\n    if (a) {\n        if (b) {\n            x=1; } }   // note\n}\n";
    check(
        lone,
        &["--style=lisp", "--pad-oper"],
        "void f() {\n    if (ready) {\n        value = a + b + c + d; } // note\n}\n",
    );
    check(
        nested,
        &["--style=lisp", "--pad-oper"],
        "void f() {\n    if (a) {\n        if (b) {\n            x = 1; } } // note\n}\n",
    );
    check(
        lone,
        &["--style=pico", "--pad-oper"],
        "void f()\n{   if (ready)\n    {   value = a + b + c + d; } // note\n}\n",
    );
    check(
        nested,
        &["--style=pico", "--pad-oper"],
        "void f()\n{   if (a)\n    {   if (b)\n        {   x = 1; } } // note\n}\n",
    );
}

#[test]
fn break_blocks_pads_after_a_kept_block_continuing_a_kept_header_block() {
    let input = "void f()\n{\n    if (ready) { value=0; }\n    { value = 0; }\n    other();\n}\n";
    check(
        input,
        &["--style=java", "--break-blocks", "--keep-one-line-blocks"],
        "void f() {\n    if (ready) { value=0; }\n\n    { value = 0; }\n\n    other();\n}\n",
    );
}

#[test]
fn break_blocks_carries_a_header_block_through_bare_blocks_after_it() {
    let input = "void f()\n{\n    while (ready) { value=0; }\n    { value = 0; }\n    { value = 1; }\n    other();\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks", "--keep-one-line-blocks"],
        "void f()\n{\n    while (ready) { value=0; }\n\n    { value = 0; }\n\n    { value = 1; }\n\n    other();\n}\n",
    );
}

#[test]
fn a_space_pico_sets_in_an_element_row_a_brace_runs_into_takes_from_its_comment_gap() {
    let narrow = "static const Node dummy = {\n  {{NULL}, EMPTY,  /* value */\n   DEADKEY, 0, {NULL}}  /* key */\n};\n";
    let wide = "static const Node dummy = {\n  {{NULL}, EMPTY,     /* value */\n   DEADKEY, 0, {NULL}}  /* key */\n};\n";
    check(
        narrow,
        &["--style=pico"],
        "static const Node dummy = {\n    {   {NULL }, EMPTY, /* value */\n        DEADKEY, 0, {NULL } } /* key */\n};\n",
    );
    check(
        wide,
        &["--style=pico"],
        "static const Node dummy = {\n    {   {NULL }, EMPTY,    /* value */\n        DEADKEY, 0, {NULL } } /* key */\n};\n",
    );
    check(
        wide,
        &["--style=kr"],
        "static const Node dummy = {\n    {   {NULL}, EMPTY,     /* value */\n        DEADKEY, 0, {NULL}\n    }  /* key */\n};\n",
    );
}

#[test]
fn breaking_one_line_headers_leaves_a_comment_inside_a_condition_directives_split() {
    let input = "void f()\n{\n    if (rc == -1) {\n        if (err != BUSY\n#if (WIN)\n            /* note */\n            && err != AGAIN\n#endif\n            )\n        {\n            {\n            }\n        }\n    }\n}\n";
    check(
        input,
        &["--style=1tbs", "--break-one-line-headers"],
        "void f()\n{\n    if (rc == -1) {\n        if (err != BUSY\n#if (WIN)\n                /* note */\n                && err != AGAIN\n#endif\n           ) {\n            {\n            }\n        }\n    }\n}\n",
    );
}

#[test]
fn pico_attaching_closing_while_attaches_a_while_on_its_own_source_line_as_other_styles_do() {
    let input = "void f()\n{\n    do {\n        value=0;\n    } while (ready);\n    do\n    {\n        value=1;\n    }\n    while (ready);\n}\n";
    check(
        input,
        &["--style=pico", "--attach-closing-while"],
        "void f()\n{   do\n    {   value=0; } while (ready);\n    do\n    {   value=1; } while (ready); }\n",
    );
    check(
        input,
        &["--style=lisp", "--attach-closing-while"],
        "void f() {\n    do {\n        value=0; } while (ready);\n    do {\n        value=1; } while (ready); }\n",
    );
}

#[test]
fn vtk_closes_a_block_a_conditional_splits_in_a_case_block_under_its_brace() {
    let input = "void f(int i)\n{\n    switch (i) {\n    case 1: {\n        if (a) {\n            x();\n#if B\n        } else if (c) {\n            y();\n#endif\n        } else {\n            z();\n        }\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=vtk"],
        "void f(int i)\n{\n    switch (i)\n        {\n        case 1:\n            {\n            if (a)\n                {\n                x();\n#if B\n                }\n            else if (c)\n                {\n                y();\n#endif\n                }\n            else\n                {\n                z();\n                }\n            break;\n            }\n        }\n}\n",
    );
}

#[test]
fn vtk_indents_an_assigned_brace_row_in_a_block_its_braces_indent() {
    let input = "Metrics g =\n    {0, a + b};\nvoid f()\n{\n    {\n        Metrics m =\n            {0, 1};\n    }\n    switch (x) {\n    case 1:\n        Metrics n =\n            {0, 1};\n        break;\n    }\n    while (x) {\n        if (y) {\n            Metrics k =\n                {0, 1};\n        }\n    }\n    Metrics other = {\n        0, a + b\n    };\n}\n";
    check(
        input,
        &["--style=vtk"],
        "Metrics g =\n{0, a + b};\nvoid f()\n{\n        {\n        Metrics m =\n            {0, 1};\n        }\n    switch (x)\n        {\n        case 1:\n            Metrics n =\n                {0, 1};\n            break;\n        }\n    while (x)\n        {\n        if (y)\n            {\n            Metrics k =\n                {0, 1};\n            }\n        }\n    Metrics other =\n        {\n        0, a + b\n        };\n}\n",
    );
}

#[test]
fn whitesmith_indents_an_assigned_brace_row_from_a_statement_after_a_case_label() {
    let input = "void f()\n{\n    switch (x) {\n    case 1:\n        Metrics n =\n            {0, 1};\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (x)\n        {\n        case 1:\n            Metrics n =\n                {0, 1};\n            break;\n        }\n    }\n",
    );
}

#[test]
fn kept_switch_labels_ending_in_a_user_label_end_their_line() {
    let input = "void f(int p) {\n  switch (p) {\n    case 1: one:\n      x();\n    case 2: case 3:\n      x();\n    default: dflt:\n      x();\n  }\n}\n";
    check(
        input,
        &["--style=kr", "--keep-one-line-statements"],
        "void f(int p)\n{\n    switch (p) {\n    case 1: one:\n        x();\n    case 2: case 3:\n        x();\n    default: dflt:\n        x();\n    }\n}\n",
    );
}

#[test]
fn break_blocks_pads_after_a_comment_led_block_of_an_else_a_conditional_splits() {
    let input = "void f()\n{\n    if (a) {\n        void *r;\n#if T\n        if (b) {\n            r = c;\n        } else\n#endif\n        /*if( !b )*/ {\n            r = d;\n        }\n        *n = r->k;\n        k = r->a;\n    } else {\n        k = 0;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks"],
        "void f()\n{\n    if (a) {\n        void *r;\n#if T\n\n        if (b) {\n            r = c;\n        } else\n#endif\n            /*if( !b )*/ {\n            r = d;\n        }\n\n        *n = r->k;\n        k = r->a;\n    } else {\n        k = 0;\n    }\n}\n",
    );
}

#[test]
fn a_switch_closer_a_comment_follows_stands_under_its_brace_after_case_blocks() {
    let input = "void f(int e)\n{\n    switch (e) {\n      case 1: {\n        x();\n        break;\n      }\n    } /* end */\n    z();\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f(int e)\n{\n    switch (e)\n    {\n    case 1:\n    {\n        x();\n        break;\n    }\n    } /* end */\n    z();\n}\n",
    );
}

#[test]
fn break_blocks_pads_before_an_else_a_comment_leads_once_its_closer_is_removed() {
    let input = "void f()\n{\n    if (ready) {\n        value=0;\n    } else if (other) { // second\n        value=1;\n    } /* last */ else {\n        value=2;\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--remove-braces", "--break-blocks=all"],
        "void f()\n{\n    if (ready)\n        value=0;\n\n    else if (other)   // second\n        value=1;\n\n    /* last */ else\n        value=2;\n}\n",
    );
}

#[test]
fn whitesmith_sets_the_body_brace_of_a_function_returning_a_defined_enum_at_its_header() {
    let input = "#include <x.h>\n\nstatic enum {\n\tFIRST,\n\tSECOND\n} config(struct repository *r)\n{\n\tconst char *str = NULL;\n\treturn FIRST;\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "#include <x.h>\n\nstatic enum\n    {\n    FIRST,\n    SECOND\n    } config(struct repository *r)\n    {\n    const char *str = NULL;\n    return FIRST;\n    }\n",
    );
}

#[test]
fn a_declarator_after_an_initializer_stands_at_its_declaration_in_a_function() {
    let input = "void f()\n{\n    static const struct Item\n        first = {\n            1, 2\n        },\n        __attribute__((unused))\n        second = {\n            3, 4\n        };\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    static const struct Item\n        first = {\n        1, 2\n    },\n    __attribute__((unused))\n    second = {\n        3, 4\n    };\n}\n",
    );
}

#[test]
fn a_comment_between_declarators_stands_at_a_continued_declaration() {
    let input = "static const struct Item\n    first = {\n        1, 2\n    },\n    /* note */\n    second = {\n        3, 4\n    };\n";
    check(
        input,
        &[],
        "static const struct Item\n    first = {\n    1, 2\n},\n/* note */\nsecond = {\n    3, 4\n};\n",
    );
}

#[test]
fn vtk_sets_a_lone_brace_of_a_continued_declarator_a_level_past_its_declaration() {
    let input = "void f()\n{\n    static const struct Item\n        first = {\n            1, 2\n        },\n        second = {\n            3, 4\n        };\n}\n";
    check(
        input,
        &["--style=vtk"],
        "void f()\n{\n    static const struct Item\n        first =\n        {\n        1, 2\n        },\n    second =\n        {\n        3, 4\n        };\n}\n",
    );
}

#[test]
fn break_blocks_pads_before_a_formatted_define_after_a_block() {
    let input = "void f()\n{\n    if (ready) {\n        first();\n    }\n#define CHECK(A) \\\n      do {           \\\n        x(A);        \\\n      } while (0)\n    CHECK(value);\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks", "--indent-preproc-define"],
        "void f()\n{\n    if (ready) {\n        first();\n    }\n\n#define CHECK(A) \\\n    do {           \\\n        x(A);        \\\n    } while (0)\n    CHECK(value);\n}\n",
    );
}

#[test]
fn break_blocks_pads_no_statement_after_an_initializer_following_a_header_block() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1:\n        break;\n    }\n    struct Item list[] = {\n        1, 2\n    };\n    helper(1);\n}\n";
    check(
        input,
        &["--style=kr", "--break-blocks"],
        "void f()\n{\n    switch (kind) {\n    case 1:\n        break;\n    }\n\n    struct Item list[] = {\n        1, 2\n    };\n    helper(1);\n}\n",
    );
}

#[test]
fn ratliff_keeps_a_block_after_a_directive_in_a_case_at_its_brace_after_an_else_chain() {
    let input = "void f()\n{\n    if (ready)\n        value=0;\n    else\n        value=2;\n    switch (state) {\n    case 1:\n#line 46 \"input.l\"\n    {\n        pop(state);\n    }\n    break;\n    }\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    if (ready)\n        value=0;\n    else\n        value=2;\n    switch (state) {\n        case 1:\n#line 46 \"input.l\"\n                {\n                pop(state);\n                }\n            break;\n        }\n    }\n",
    );
}

#[test]
fn a_nested_switch_body_after_an_element_table_stands_a_level_past_its_label() {
    let input = "static const struct Item items[] = {\n    { 1 },\n};\n\nvoid f()\n{\n    switch (kind) {\n    case 1: {\n        switch (other) {\n        case 2:\n            value = 2;\n            break;\n        }\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
    check(
        input,
        &["--style=allman"],
        "static const struct Item items[] = {\n    { 1 },\n};\n\nvoid f()\n{\n    switch (kind)\n    {\n    case 1:\n    {\n        switch (other)\n        {\n        case 2:\n            value = 2;\n            break;\n        }\n        break;\n    }\n    }\n}\n",
    );
}

#[test]
fn a_nested_switch_body_after_a_table_in_a_case_block_stands_a_level_past_its_label() {
    let input = "void f()\n{\n    switch (kind) {\n    case 2:\n    {\n        static const struct Item other[] = {\n            { 1 },\n        };\n        switch (other) {\n        case 2:\n            value = 2;\n            break;\n        }\n    }\n    }\n}\n";
    check(input, &[], input);
    check(
        input,
        &["--indent-switches"],
        "void f()\n{\n    switch (kind) {\n        case 2:\n        {\n            static const struct Item other[] = {\n                { 1 },\n            };\n            switch (other) {\n                case 2:\n                    value = 2;\n                    break;\n            }\n        }\n    }\n}\n",
    );
}

#[test]
fn keeping_one_line_statements_keeps_a_statement_after_an_empty_block_on_its_line() {
    let input = "void f()\n{\n    if (ready) {} next();\n    while (busy()) {} next();\n    {} next();\n    switch (x) {} next();\n    do {} while (x); next();\n    if (a) {} else {} next();\n    for (;;) {} for (;;) {}\n}\n";
    check(input, &["--style=kr", "--keep-one-line-statements"], input);
    check(
        input,
        &["--style=allman", "--keep-one-line-statements"],
        "void f()\n{\n    if (ready) {} next();\n    while (busy()) {} next();\n    {} next();\n    switch (x) {} next();\n    do {}\n    while (x); next();\n    if (a) {}\n    else {} next();\n    for (;;) {} for (;;) {}\n}\n",
    );
}

#[test]
fn styles_breaking_closing_braces_break_the_while_off_an_empty_do_block() {
    let input = "void f()\n{\n    do {} while (x);\n    next();\n}\n";
    let broken = "void f()\n{\n    do {}\n    while (x);\n    next();\n}\n";
    check(input, &["--style=stroustrup"], broken);
    check(input, &["--break-closing-braces"], broken);
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    do {}\n    while (x);\n    next();\n    }\n",
    );
    check(input, &["--style=kr"], input);
}

#[test]
fn run_in_closers_take_the_padding_before_a_commented_else_ahead_of_the_row_they_join() {
    let input = "void f()\n{\n    if (ready) {\n        first();\n        value=0;\n    } /* c */ else {\n        result=0;\n    }\n    if (ready) {\n        value=0; // note\n    } /* c */ else {\n        result=0;\n    }\n    next();\n}\n";
    check(
        input,
        &["--style=lisp", "--break-blocks=all"],
        "void f() {\n    if (ready) {\n        first();\n\n        value=0; } /* c */ else {\n\n        result=0; }\n\n    if (ready) {\n        value=0; // note\n\n    } /* c */ else {\n\n        result=0; }\n\n    next(); }\n",
    );
    check(
        input,
        &["--style=pico", "--break-blocks=all"],
        "void f()\n{   if (ready)\n    {   first();\n\n        value=0; } /* c */ else\n\n    {   result=0; }\n\n    if (ready)\n    {   value=0; // note\n\n    } /* c */ else\n\n    {   result=0; }\n\n    next(); }\n",
    );
}

#[test]
fn lisp_pads_after_a_commented_else_if_a_closer_joins_as_after_a_commented_else() {
    let input = "void f()\n{\n    if (ready) {\n        value=0;\n    } /* c */ else if (other) {\n        result=0;\n    } /* d */ else {\n        result=1;\n    }\n}\n";
    check(
        input,
        &["--style=lisp", "--break-blocks"],
        "void f() {\n    if (ready) {\n        value=0; } /* c */ else if (other) {\n\n        result=0; } /* d */ else {\n\n        result=1; } }\n",
    );
}

#[test]
fn ratliff_sets_broken_element_rows_of_a_table_in_a_case_body_as_outside_a_switch() {
    let input = "void f()\n{\n    switch (op) {\n    default: {\n        int table[][2] = {\n            { FIRST,  1 },\n            {\n                SECOND, 2|\n                4\n            },\n        };\n        break;\n    }\n    case 2:\n        int other[][2] = {\n            {\n                SECOND, 2|\n                4\n            },\n        };\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    switch (op) {\n        default: {\n            int table[][2] = {\n                    { FIRST,  1 },\n                    {\n                    SECOND, 2|\n                    4\n                    },\n                };\n            break;\n            }\n        case 2:\n            int other[][2] = {\n                    {\n                    SECOND, 2|\n                    4\n                    },\n                };\n            break;\n        }\n    }\n",
    );
}

#[test]
fn a_comment_leading_a_case_label_line_leaves_the_label_at_the_case_column() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1:\n        value = 1;\n    /* fall through */ case 2:\n        value = 2;\n        break;\n    /* other */ default:\n        break;\n    }\n}\n";
    check(input, &[], input);
    check(
        input,
        &["--indent-switches"],
        "void f()\n{\n    switch (kind) {\n        case 1:\n            value = 1;\n        /* fall through */ case 2:\n            value = 2;\n            break;\n        /* other */ default:\n            break;\n    }\n}\n",
    );
}

#[test]
fn gnu_sets_the_case_block_brace_of_a_comment_led_label_under_the_label() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value = 1;\n        break;\n    }\n    /*case 2*/ default: {\n        if (ready) {\n            start();\n        }\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=gnu"],
        "void f()\n{\n    switch (kind)\n        {\n        case 1:\n        {\n            value = 1;\n            break;\n        }\n        /*case 2*/ default:\n        {\n            if (ready)\n                {\n                    start();\n                }\n            break;\n        }\n        }\n}\n",
    );
}

#[test]
fn keeping_one_line_statements_keeps_a_label_after_a_statement_on_its_line() {
    let input = "void f()\n{\n    value = 1; again:\n    next();\n    if (ready) {\n        a(); stop:\n        b();\n    }\n}\n";
    check(input, &["--keep-one-line-statements"], input);
}

#[test]
fn a_kept_line_ending_in_a_label_stays_at_the_column_of_its_first_statement() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: one:\n        value = 1;\n        break;\n    case 2: value = 2; two:\n        break;\n    }\n}\n";
    check(input, &["--keep-one-line-statements"], input);
    check(
        input,
        &["--keep-one-line-statements", "--indent-labels"],
        input,
    );
}

#[test]
fn a_declaration_after_a_define_ending_in_a_closer_continues_its_declarator_row() {
    let input = "#define CHECK(v) \\\n    do { \\\n        h(v); \\\n    } while (0)\n\nstatic const struct Item\n    first = {\n        1, 2\n    };\n";
    check(
        input,
        &["--style=kr"],
        "#define CHECK(v) \\\n    do { \\\n        h(v); \\\n    } while (0)\n\nstatic const struct Item\n    first = {\n    1, 2\n};\n",
    );
}

#[test]
fn padding_options_reach_the_while_of_an_empty_do_block() {
    let input = "void f()\n{\n    do {} while(x);\n    do {} while ( x );\n    next();\n}\n";
    check(
        input,
        &["--unpad-paren"],
        "void f()\n{\n    do {} while(x);\n    do {} while(x);\n    next();\n}\n",
    );
    check(
        input,
        &["--pad-header"],
        "void f()\n{\n    do {} while (x);\n    do {} while ( x );\n    next();\n}\n",
    );
}

#[test]
fn a_case_block_after_a_comment_led_label_stands_as_after_a_plain_label() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1:\n        break;\n    /* other */ default: {\n        value = 2;\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (kind)\n        {\n        case 1:\n            break;\n        /* other */ default:\n            {\n            value = 2;\n            break;\n            }\n        }\n    }\n",
    );
    check(
        input,
        &["--style=ratliff", "--indent-switches"],
        "void f() {\n    switch (kind) {\n        case 1:\n            break;\n        /* other */ default: {\n            value = 2;\n            break;\n            }\n        }\n    }\n",
    );
}

#[test]
fn a_parameter_paren_split_off_a_long_file_scope_header_stands_where_it_would_on_its_own_line() {
    let input = "Decl(jint,first_very_long_function_name_component_that_goes_past_the_limit_of_eighty_xyz_abc)(\n    int a, int b\n) {\n    return 1;\n}\n";
    let expected = "Decl(jint,\n     first_very_long_function_name_component_that_goes_past_the_limit_of_eighty_xyz_abc)\n(\n    int a, int b\n) {\n    return 1;\n}\n";
    check(input, &["--style=google", "--max-code-length=80"], expected);
    check(
        expected,
        &["--style=google", "--max-code-length=80"],
        expected,
    );
}

#[test]
fn whitesmith_sets_a_comment_led_label_brace_on_its_own_line_as_a_case_block() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1:\n        break;\n    /* other */ default:\n    {\n        if (x)\n            y();\n        value = 2;\n        break;\n    }\n    }\n}\n";
    let expected = "void f()\n    {\n    switch (kind)\n        {\n        case 1:\n            break;\n        /* other */ default:\n            {\n            if (x)\n                y();\n            value = 2;\n            break;\n            }\n        }\n    }\n";
    check(input, &["--style=whitesmith"], expected);
    check(expected, &["--style=whitesmith"], expected);
}

#[test]
fn a_comment_leading_a_label_line_leaves_it_a_label() {
    let input = "void f()\n{\n    /* note */ done: value=0;\n    /* note */ again:\n    next();\n    if (x) {\n        /* note */ stop: y();\n    }\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n/* note */ done:\n    value=0;\n/* note */ again:\n    next();\n    if (x) {\n/* note */ stop:\n        y();\n    }\n}\n",
    );
    check(
        input,
        &["--indent-labels"],
        "void f()\n{\n/* note */ done:\n    value=0;\n/* note */ again:\n    next();\n    if (x) {\n    /* note */ stop:\n        y();\n    }\n}\n",
    );
    check(
        input,
        &["--keep-one-line-statements"],
        "void f()\n{\n/* note */ done: value=0;\n/* note */ again:\n    next();\n    if (x) {\n/* note */ stop: y();\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_an_expanded_block_follows_its_first_statement_when_braces_are_added() {
    let input = "void f()\n{\n    for (;;) { if (ready) break; else continue; } /* note */\n}\n";
    check(
        input,
        &["--style=java", "--add-braces"],
        "void f() {\n    for (;;) {\n        if (ready) {\n            break;    /* note */\n        } else {\n            continue;\n        }\n    }\n}\n",
    );
}

#[test]
fn lisp_moves_a_comment_after_an_expanded_block_only_off_rows_no_closer_joins() {
    let input = "void f()\n{\n    for (;;) { if (ready) break; else continue; } /* note */\n    for (;;) { a(); b(); } /* note */\n    if (x) { a(); } /* note */\n}\n";
    let expected = "void f() {\n    for (;;) {\n        if (ready) {\n            break; }\n        else {\n            continue; } } /* note */\n    for (;;) {\n        a();    /* note */\n        b(); }\n    if (x) {\n        a(); } /* note */\n}\n";
    check(input, &["--style=lisp", "--add-braces"], expected);
    check(input, &["--style=lisp", "--add-one-line-braces"], expected);
}

#[test]
fn a_line_splice_after_a_statement_stays_on_its_line_when_statements_are_kept() {
    let input = "void f()\n{\n    fail();    \\\n    next();\n}\n";
    check(input, &["--keep-one-line-statements"], input);
    check(
        input,
        &["--style=lisp"],
        "void f() {\n    fail();    \\\n    next(); }\n",
    );
}

#[test]
fn a_comment_leading_a_continuation_row_leaves_the_row_continued() {
    // The comment changes nothing about the code after it.
    let input = "static const struct Item\n    /* note */ first = {\n    1, 2\n};\n";
    check(input, &["--style=google"], input);
}

#[test]
fn breaking_return_types_breaks_a_definition_a_comment_leads() {
    let input = "/* note */ void f()\n{\n    run();\n}\n";
    check(
        input,
        &["--style=kr", "--break-return-type"],
        "/* note */ void\nf()\n{\n    run();\n}\n",
    );
}

#[test]
fn a_constructor_member_after_a_directive_aligns_with_the_members_before_it() {
    let input = "class Item {\npublic:\n    Item(int value, int other)\n#ifdef FEATURE\n        : value_(value),\n#endif\n          other_(other) {}\n};\n";
    check(
        input,
        &["--style=kr"],
        "class Item\n{\npublic:\n    Item(int value, int other)\n#ifdef FEATURE\n        : value_(value),\n#endif\n          other_(other) {}\n};\n",
    );
}

#[test]
fn run_in_styles_align_constructor_members_after_an_access_label() {
    let input = "class Item {\npublic:\n    Item(int value, int other)\n        : value_(value),\n          other_(other) {}\n};\n";
    check(
        input,
        &["--style=horstmann"],
        "class Item\n{\npublic:\n    Item(int value, int other)\n        : value_(value),\n          other_(other) {}\n};\n",
    );
    check(
        input,
        &["--style=pico"],
        "class Item\n{\npublic:\n    Item(int value, int other)\n        : value_(value),\n          other_(other) {} };\n",
    );
}

#[test]
fn a_comment_leading_a_function_brace_leaves_the_brace_a_function_body() {
    let input = "void f()\n/* note */ {\n    if (x)\n    /* c */ {\n        y();\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n/* note */\n{\n    if (x)\n        /* c */ {\n        y();\n    }\n}\n",
    );
    check(
        input,
        &["--style=java"],
        "void f()\n/* note */ {\n    if (x)\n        /* c */ {\n        y();\n    }\n}\n",
    );
    let blank = "void f()\n/* note */ {\n    result = ((first + (second * 2))\n              - third) / (fourth\n\n                          + fifth);\n}\n";
    check(
        blank,
        &["--style=gnu", "--delete-empty-lines"],
        "void f()\n/* note */\n{\n    result = ((first + (second * 2))\n              - third) / (fourth\n                          + fifth);\n}\n",
    );
}

#[test]
fn the_while_of_a_do_a_comment_leads_stands_at_the_do_line() {
    let input = "void f()\n{\n    /* note */ do {\n        value=0;\n    } while (ready);\n    /* note */ do\n        value=0;\n    while (ready);\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    /* note */ do\n    {\n        value=0;\n    }\n    while (ready);\n    /* note */ do\n        value=0;\n    while (ready);\n}\n",
    );
    check(input, &["--style=kr"], input);
}

#[test]
fn a_statement_after_a_comment_led_label_in_a_directive_stands_in_its_block() {
    let input = "void f()\n{\n    if (ready) {\n#ifdef FEATURE\n    /* note */ retry:\n#endif\n        value = 1;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    if (ready) {\n#ifdef FEATURE\n/* note */ retry:\n#endif\n        value = 1;\n    }\n}\n",
    );
}

#[test]
fn a_statement_broken_off_its_label_keeps_the_gap_before_its_comment() {
    let input = "void f()\n{\n    done: value=0;   /* note */\n    again: value=0; // note\n}\nclass Item\n{\n    public: int value; /* note */\n};\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\ndone:\n    value=0;   /* note */\nagain:\n    value=0; // note\n}\nclass Item\n{\npublic:\n    int value; /* note */\n};\n",
    );
    check(
        input,
        &["--style=kr", "--pad-oper"],
        "void f()\n{\ndone:\n    value = 0; /* note */\nagain:\n    value = 0; // note\n}\nclass Item\n{\npublic:\n    int value; /* note */\n};\n",
    );
}

#[test]
fn a_comment_leading_the_first_case_label_leaves_it_at_the_case_column() {
    let input = "void f()\n{\n    switch (kind) {\n    /* note */ case 1:\n        value = 1;\n        break;\n    default:\n        break;\n    }\n}\n";
    check(input, &["--style=1tbs"], input);
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    switch (kind)\n    {\n    /* note */ case 1:\n        value = 1;\n        break;\n    default:\n        break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=1tbs", "--indent-switches"],
        "void f()\n{\n    switch (kind) {\n        /* note */ case 1:\n            value = 1;\n            break;\n        default:\n            break;\n    }\n}\n",
    );
}

#[test]
fn unpadding_a_header_broken_off_a_statement_widens_the_gap_before_its_comment() {
    let input = "void f()\n{\n    a(); while (x) { /* note */\n        first();\n    }\n    if (ready) helper(); else switch (state) { /* note */\n    case 1:\n        first();\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=linux", "--unpad-paren"],
        "void f()\n{\n    a();\n    while(x) {  /* note */\n        first();\n    }\n    if(ready) helper();\n    else switch(state) {  /* note */\n        case 1:\n            first();\n            break;\n        }\n}\n",
    );
    check(
        input,
        &["--style=1tbs", "--unpad-paren"],
        "void f()\n{\n    a();\n    while(x) {  /* note */\n        first();\n    }\n    if(ready) {\n        helper();\n    } else switch(state) {  /* note */\n        case 1:\n            first();\n            break;\n        }\n}\n",
    );
}

#[test]
fn a_comment_after_a_brace_stays_on_its_line_after_a_one_line_block_before_it() {
    let input = "void f()\n{\n    if (ready) { helper(); } else { /* note */\n        first();\n    }\n    while (ready) { helper(); } if (state) { /* note */\n        first();\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    if (ready) {\n        helper();\n    } else { /* note */\n        first();\n    }\n    while (ready) {\n        helper();\n    }\n    if (state) { /* note */\n        first();\n    }\n}\n",
    );
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    if (ready)\n    {\n        helper();\n    }\n    else     /* note */\n    {\n        first();\n    }\n    while (ready)\n    {\n        helper();\n    }\n    if (state)   /* note */\n    {\n        first();\n    }\n}\n",
    );
}

#[test]
fn a_broken_brace_comment_after_a_statement_keeps_its_gap() {
    let input = "void f()\n{\n    if (state) { /* note */\n        first();\n    }\n    a(); if (state) { /* note */\n        first();\n    }\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    if (state)   /* note */\n    {\n        first();\n    }\n    a();\n    if (state)   /* note */\n    {\n        first();\n    }\n}\n",
    );
}

#[test]
fn unpadding_the_row_an_added_closer_joins_widens_the_gap_before_its_comment() {
    let input = "void f()\n{\n    if (ready) helper (state); /* note */\n    if (ready) helper (state)+1;   /* note */\n}\n";
    check(
        input,
        &["--style=lisp", "--add-braces", "--unpad-paren"],
        "void f() {\n    if(ready) {\n        helper(state); }  /* note */\n    if(ready) {\n        helper(state)+1; }    /* note */\n}\n",
    );
    check(
        input,
        &[
            "--style=lisp",
            "--add-braces",
            "--unpad-paren",
            "--pad-oper",
        ],
        "void f() {\n    if(ready) {\n        helper(state); }  /* note */\n    if(ready) {\n        helper(state) + 1; }  /* note */\n}\n",
    );
    check(
        input,
        &["--style=pico", "--add-braces", "--unpad-paren"],
        "void f()\n{   if(ready) { helper(state); }   /* note */\n    if(ready) { helper(state)+1; }     /* note */\n}\n",
    );
    check(
        input,
        &[
            "--style=pico",
            "--add-braces",
            "--unpad-paren",
            "--pad-oper",
        ],
        "void f()\n{   if(ready) { helper(state); }   /* note */\n    if(ready) { helper(state) + 1; }   /* note */\n}\n",
    );
}

#[test]
fn the_condition_rows_of_a_while_on_an_indented_closer_align_after_its_paren() {
    let input = "void f()\n{\n    do {\n        value++;\n    } while (value < count &&\n             ready);\n}\n";
    check(
        input,
        &["--style=vtk", "--attach-closing-while"],
        "void f()\n{\n    do\n        {\n        value++;\n        } while (value < count &&\n                 ready);\n}\n",
    );
    check(
        input,
        &["--style=gnu", "--attach-closing-while"],
        "void f()\n{\n    do\n        {\n            value++;\n        } while (value < count &&\n                 ready);\n}\n",
    );
}

#[test]
fn a_function_brace_after_a_directive_closing_its_header_opens_a_function_body() {
    let input = "#ifdef FEATURE\nvoid f()\n#endif\n{\n    { value=0; }\n}\n";
    let expected = "#ifdef FEATURE\nvoid f()\n#endif\n{\n    {\n        value=0;\n    }\n}\n";
    check(input, &["--style=kr"], expected);
    check(input, &["--style=allman"], expected);
    check(
        input,
        &["--style=ratliff"],
        "#ifdef FEATURE\nvoid f()\n#endif\n    {\n        {\n        value=0;\n        }\n    }\n",
    );
}

#[test]
fn a_block_comment_after_a_case_brace_on_its_own_line_opens_the_block() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1:\n        { /* note */\n            value=0;\n        }\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (kind) {\n    case 1: {\n        /* note */\n        value=0;\n    }\n    break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=lisp"],
        "void f() {\n    switch (kind) {\n    case 1: {\n        /* note */\n        value=0; }\n    break; } }\n",
    );
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    switch (kind) {\n        case 1: {\n            /* note */\n            value=0;\n            }\n        break;\n        }\n    }\n",
    );
}

#[test]
fn the_statement_after_a_label_breaks_off_the_comments_after_the_label() {
    let input = "void f()\n{\n    switch (op) {\n    default:\n        /* note */ break;\n    case 1: /* note */ break;\n    case 2:\n        // line\n        /* note */ break;\n    case 3:\n        value = 0;\n        /* note */ break;\n    }\ndone:\n    /* note */ value = 1;\n}\nclass Item {\npublic:\n    /* note */ int value;\n};\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (op) {\n    default:\n        /* note */\n        break;\n    case 1: /* note */\n        break;\n    case 2:\n        // line\n        /* note */\n        break;\n    case 3:\n        value = 0;\n        /* note */ break;\n    }\ndone:\n    /* note */\n    value = 1;\n}\nclass Item\n{\npublic:\n    /* note */\n    int value;\n};\n",
    );
    check(
        input,
        &["--style=kr", "--keep-one-line-statements"],
        "void f()\n{\n    switch (op) {\n    default:\n        /* note */ break;\n    case 1: /* note */ break;\n    case 2:\n        // line\n        /* note */ break;\n    case 3:\n        value = 0;\n        /* note */ break;\n    }\ndone:\n    /* note */ value = 1;\n}\nclass Item\n{\npublic:\n    /* note */ int value;\n};\n",
    );
}

#[test]
fn a_brace_or_a_directive_after_the_comments_after_a_label_ends_its_reach() {
    let input = "void f()\n{\n    switch (op) {\n    case 1: /* a */ /* b */ break;\n    case 2: // line\n        break;\n    case 3: /* a */ { value = 0; }\n    case 4:\n        /* a */ { value = 0; }\n    case 5: /* a */\n#if X\n        /* b */ value = 1;\n#endif\n        break;\n    }\nagain: /* c */ value = 2; /* d */\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (op) {\n    case 1: /* a */ /* b */\n        break;\n    case 2: // line\n        break;\n    case 3: { /* a */\n        value = 0;\n    }\n    case 4:\n        /* a */\n    {\n        value = 0;\n    }\n    case 5: /* a */\n#if X\n        /* b */ value = 1;\n#endif\n        break;\n    }\nagain: /* c */\n    value = 2; /* d */\n}\n",
    );
}

#[test]
fn a_comment_led_statement_after_a_conditional_after_a_case_label_stands_in_its_body() {
    let input = "void f()\n{\n    switch (op) {\n    case 5:\n#if X\n        /* b */ value = 1;\n#endif\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    switch (op) {\n        case 5:\n#if X\n            /* b */ value = 1;\n#endif\n            break;\n        }\n    }\n",
    );
    check(input, &["--style=kr"], input);
}

#[test]
fn a_kept_label_line_with_a_comment_after_the_label_stands_at_the_label_column() {
    let input = "void f()\n{\n    value = 1;\nagain: value = 2;\nretry: /* c */ value = 2;\n}\n";
    check(input, &["--style=kr", "--keep-one-line-statements"], input);
    check(
        input,
        &["--style=lisp"],
        "void f() {\n    value = 1;\nagain: value = 2;\nretry: /* c */ value = 2; }\n",
    );
}

#[test]
fn a_comment_led_statement_after_comments_after_a_case_label_stands_in_its_body() {
    let input = "void f()\n{\n    switch (op) {\n    case 1: break;\n    case 2:\n        // line\n        /* note */ break;\n    }\n}\n";
    check(
        input,
        &["--style=ratliff", "--keep-one-line-statements"],
        "void f() {\n    switch (op) {\n        case 1: break;\n        case 2:\n            // line\n            /* note */ break;\n        }\n    }\n",
    );
}

#[test]
fn a_kept_one_line_block_after_a_comment_after_a_case_label_takes_a_line_of_its_own() {
    let input = "void f()\n{\n    if (ready) /* a */ { value = 0; }\n    switch (op) {\n    case 3: { value = 0; }\n    case 4: /* a */ { value = 0; }\n    case 5: /* a */ {\n        value = 0;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--keep-one-line-blocks"],
        "void f()\n{\n    if (ready) /* a */ { value = 0; }\n    switch (op) {\n    case 3:\n    { value = 0; }\n    case 4: /* a */\n    { value = 0; }\n    case 5: { /* a */\n        value = 0;\n    }\n    }\n}\n",
    );
    check(
        input,
        &["--style=allman", "--keep-one-line-blocks"],
        "void f()\n{\n    if (ready) /* a */ { value = 0; }\n    switch (op)\n    {\n    case 3:\n    { value = 0; }\n    case 4: /* a */\n    { value = 0; }\n    case 5: /* a */\n    {\n        value = 0;\n    }\n    }\n}\n",
    );
}

#[test]
fn a_message_row_after_a_conditional_aligns_as_after_the_row_before_it() {
    let input = "- (int)total\n{\n    return [[self items] count] + [self.other countWith:1\n#ifdef FEATURE\n                                                         and:2];\n#endif\n}\n";
    check(
        input,
        &["--style=kr"],
        "- (int)total\n{\n    return [[self items] count] + [self.other countWith:1\n#ifdef FEATURE\n                                              and:2];\n#endif\n}\n",
    );
}

#[test]
fn a_comment_split_off_an_initializer_closer_stands_at_the_row_before_it() {
    let input = "void f()\n{\n    static const struct Item\n        first = {\n            1, 2\n        /* note */ };\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    static const struct Item\n        first = {\n        1, 2\n        /* note */\n    };\n}\n",
    );
}

#[test]
fn a_comment_between_a_split_return_type_and_its_name_leaves_the_name_unindented() {
    let input = "static struct Item *\n// note\nmake(int value);\nstatic struct Item *\n/* note */\nmake(int value);\n";
    check(input, &["--style=kr"], input);
    check(input, &["--style=ratliff"], input);
}

#[test]
fn an_element_brace_after_a_comma_keeps_its_row_with_a_comment_ending_it() {
    let input = "void f()\n{\n    int table[][2] = {\n        5, { /* note */\n            6, 7\n        }\n    };\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    int table[][2] =\n    {\n        5, { /* note */\n            6, 7\n        }\n    };\n}\n",
    );
    check(
        input,
        &["--style=horstmann"],
        "void f()\n{   int table[][2] =\n    {   5, { /* note */\n            6, 7\n        }\n    };\n}\n",
    );
}

#[test]
fn a_one_line_initializer_after_a_blank_line_after_its_assignment_stays_on_its_line() {
    let input = "void f()\n{\n    struct Item other =\n\n    { 3, 4 };\n}\n";
    check(input, &["--style=kr"], input);
    check(input, &["--style=allman"], input);
}

#[test]
fn a_comment_ending_a_switch_run_in_after_an_else_stays_in_its_case_body() {
    let input = "void f()\n{\n    if (ready) {\n        value=1;\n    } else switch (kind) {\n    case 1:\n        // first\n        break;\n    case 2:\n        value = 2;\n        // note\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    if (ready) {\n        value=1;\n    } else switch (kind) {\n        case 1:\n            // first\n            break;\n        case 2:\n            value = 2;\n            // note\n        }\n}\n",
    );
}

#[test]
fn an_operator_led_row_in_parentheses_after_a_blank_line_aligns_in_them() {
    let input = "void f()\n{\n    out << helper(first, (second\n\n                          << third));\n}\n";
    check(input, &["--style=kr"], input);
    check(
        input,
        &["--style=vtk"],
        "void f()\n{\n    out << helper(first, (second\n\n                          << third));\n}\n",
    );
}

#[test]
fn a_message_a_comment_leads_aligns_its_rows_past_the_comment() {
    let input = "- (void)update\n{\n    /* note */ [self apply:value\n           with:other];\n}\n";
    check(
        input,
        &["--style=linux"],
        "- (void)update\n{\n    /* note */ [self apply:value\n                     with:other];\n}\n",
    );
}

#[test]
fn a_comment_split_off_an_initializer_closer_after_an_element_stands_at_the_list_body() {
    let input = "void f()\n{\n    switch (op) {\n    default: {\n        int table[][2] = {\n            { FIRST, 1 },\n            {\n                SECOND, 2\n            },\n        /* note */ };\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (op) {\n    default: {\n        int table[][2] = {\n            { FIRST, 1 },\n            {\n                SECOND, 2\n            },\n            /* note */\n        };\n        break;\n    }\n    }\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (op)\n        {\n        default:\n            {\n            int table[][2] =\n                {\n                    { FIRST, 1 },\n                    {\n                    SECOND, 2\n                    },\n                /* note */\n                };\n            break;\n            }\n        }\n    }\n",
    );
}

#[test]
fn a_comment_leading_a_switch_line_leaves_the_comment_after_its_brace_as_without_it() {
    let input = "void f()\n{\n    /* note */ switch (kind) {  /* dispatch on kind\n                    ** of value */\n    case 1:\n        value = 0;\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    /* note */ switch (kind) {\n    /* dispatch on kind\n                    ** of value */\n    case 1:\n        value = 0;\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_one_line_initializer_after_a_conditional_after_its_assignment_stays_on_its_line() {
    let input = "void f()\n{\n    struct Item item =\n    { 1, 2 };\n    struct Item next =\n#ifdef FEATURE\n    { 3, 4 };\n#endif\n}\n";
    check(input, &[], input);
    check(input, &["--style=kr"], input);
}

#[test]
fn comments_after_a_case_block_stand_in_the_case_body() {
    let input = "void f()\n{\n    switch (state) {\n    case 1: {\n        first();\n    }\n    // after block\n    second();\n    // after statement\n    third();\n    /* block */\n    break;\n    default:\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    switch (state)\n    {\n    case 1:\n    {\n        first();\n    }\n        // after block\n    second();\n        // after statement\n    third();\n        /* block */\n    break;\n    default:\n        break;\n    }\n}\n",
    );
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (state) {\n    case 1: {\n        first();\n    }\n        // after block\n    second();\n        // after statement\n    third();\n        /* block */\n    break;\n    default:\n        break;\n    }\n}\n",
    );
    let closing = "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0;\n        break;\n    }\n    default: {\n        break;\n    }\n    // note\n    }\n}\n";
    check(
        closing,
        &["--style=allman"],
        "void f()\n{\n    switch (kind)\n    {\n    case 1:\n    {\n        value=0;\n        break;\n    }\n    default:\n    {\n        break;\n    }\n        // note\n    }\n}\n",
    );
    check(
        closing,
        &["--style=gnu"],
        "void f()\n{\n    switch (kind)\n        {\n        case 1:\n        {\n            value=0;\n            break;\n        }\n        default:\n        {\n            break;\n        }\n            // note\n        }\n}\n",
    );
}

#[test]
fn a_comment_after_a_constructor_initializer_stands_at_its_members() {
    let input = "Item::Item(int value)\n    : value(value), other(0)\n/* note */ {\n}\nItem::Item(int value)\n    : value(value),\n      other(0)\n  // note\n{\n}\nItem::Item(int value)\n    : value(value),\n// between\n      other(0)\n{\n}\n";
    check(
        input,
        &["--style=linux"],
        "Item::Item(int value)\n    : value(value), other(0)\n      /* note */\n{\n}\nItem::Item(int value)\n    : value(value),\n      other(0)\n      // note\n{\n}\nItem::Item(int value)\n    : value(value),\n// between\n      other(0)\n{\n}\n",
    );
}

#[test]
fn a_member_after_a_conditional_member_after_an_access_label_stands_at_the_members() {
    let input = "class Item {\npublic:\n#ifdef FEATURE\n    int value;\n#endif\n    Item() : value(0) {}\n    int count;\n};\n";
    check(
        input,
        &["--style=kr"],
        "class Item\n{\npublic:\n#ifdef FEATURE\n    int value;\n#endif\n    Item() : value(0) {}\n    int count;\n};\n",
    );
    check(
        input,
        &["--style=google"],
        "class Item {\n  public:\n#ifdef FEATURE\n    int value;\n#endif\n    Item() : value(0) {}\n    int count;\n};\n",
    );
}

#[test]
fn a_template_head_a_comment_leads_aligns_its_parameters_past_the_comment() {
    let input = "/* note */ template <typename First,\n                     typename Second>\nclass Pair\n{\n};\n";
    check(input, &["--style=kr"], input);
    check(
        input,
        &["--style=whitesmith"],
        "/* note */ template <typename First,\n                     typename Second>\nclass Pair\n    {\n    };\n",
    );
}

#[test]
fn the_statement_after_a_comment_led_case_label_breaks_off_the_comments_after_it() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1:\n        value = 1;\n    /* fall through */ case 2:\n        /* note */ value = 2;\n        break;\n    /* other */ default: /* c */ value = 3;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (kind) {\n    case 1:\n        value = 1;\n    /* fall through */ case 2:\n        /* note */\n        value = 2;\n        break;\n    /* other */ default: /* c */\n        value = 3;\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_an_element_closer_leaves_the_closer_where_it_stands_without_it() {
    let input = "void f()\n{\n    static const struct Item\n        first = {\n            1, 2\n        }, /* note */\n        second = {\n            3, 4\n        };\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    static const struct Item\n        first = {\n        1, 2\n    }, /* note */\n    second = {\n        3, 4\n    };\n}\n",
    );
}

#[test]
fn a_one_line_initializer_after_a_commented_assignment_keeps_its_line() {
    let input = "void f()\n{\n    struct Item item = /* note */\n        { 1, 2 };\n}\n";
    check(
        input,
        &["--style=linux"],
        "void f()\n{\n    struct Item item = /* note */\n    { 1, 2 };\n}\n",
    );
}

#[test]
fn a_template_head_a_comment_leads_realigns_its_parameters_past_the_comment() {
    let input = "template <typename First,\n     typename Second>\nclass Pair\n{\n};\n/* note */ template <typename First,\n     typename Second>\nclass Other\n{\n};\n/* note */ template <typename First,\n          typename Second>\nclass Last\n{\n};\n";
    check(
        input,
        &["--style=kr"],
        "template <typename First,\n          typename Second>\nclass Pair\n{\n};\n/* note */ template <typename First,\n                     typename Second>\nclass Other\n{\n};\n/* note */ template <typename First,\n                     typename Second>\nclass Last\n{\n};\n",
    );
}

#[test]
fn a_stream_row_after_a_blank_line_aligns_in_its_chain() {
    let input = "void f()\n{\n    std::cout << first\n\n              << second << std::endl;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_comment_after_blocks_broken_off_one_line_follows_the_block_it_ends() {
    let input = "void f()\n{\n    if (ready) { value=0; } else { result=0; } /* note */\n}\n";
    check(
        input,
        &["--style=1tbs"],
        "void f()\n{\n    if (ready) {\n        value=0;\n    } else {\n        result=0;    /* note */\n    }\n}\n",
    );
}

#[test]
fn an_element_brace_after_a_comment_alone_opens_its_row_as_one_leading_it() {
    let input = "static const struct Item items[] = {\n    {1, 2},\n    /* note */ {3,\n     4},\n    {5,\n     6}\n};\n";
    check(
        input,
        &["--style=kr"],
        "static const struct Item items[] = {\n    {1, 2},\n    /* note */ {\n        3,\n        4\n    },\n    {\n        5,\n        6\n    }\n};\n",
    );
    check(
        input,
        &["--style=allman"],
        "static const struct Item items[] = {\n    {1, 2},\n    /* note */ {\n        3,\n        4\n    },\n    {\n        5,\n        6\n    }\n};\n",
    );
}

#[test]
fn a_designated_row_after_a_blank_line_aligns_in_its_list() {
    let input = "void f()\n{\n    return (struct Item) {\n        .value = 1,\n\n        .other = 2\n    };\n}\n";
    check(input, &["--style=stroustrup"], input);
}

#[test]
fn breaking_return_types_breaks_a_definition_a_conditional_parts_from_its_body() {
    let input = "#ifdef FEATURE\nvoid f()\n#endif\n{\n    run();\n}\n";
    check(
        input,
        &["--style=1tbs", "--break-return-type"],
        "#ifdef FEATURE\nvoid\nf()\n#endif\n{\n    run();\n}\n",
    );
}

#[test]
fn a_class_brace_a_comment_leads_opens_its_body_as_any() {
    let input = "class Item\n/* note */ {\npublic:\n    Item();\n};\n";
    check(input, &[], input);
}

#[test]
fn a_comment_split_off_an_initializer_closer_after_a_conditional_stands_at_its_rows() {
    let input = "void f()\n{\n    static const struct Item items[] = {\n#if FEATURE\n        {1, 2}\n#else\n        {3, 4}\n#endif\n    /* note */ };\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    static const struct Item items[] = {\n#if FEATURE\n        {1, 2}\n#else\n        {3, 4}\n#endif\n        /* note */\n    };\n}\n",
    );
}

#[test]
fn deleting_empty_lines_keeps_those_of_an_enum_list_as_of_an_array() {
    let input = "void f()\n{\n    enum Kind {\n\n        First,\n\n        Third\n    };\n    if (ready) {\n\n        run();\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--delete-empty-lines"],
        "void f()\n{\n    enum Kind {\n\n        First,\n\n        Third\n    };\n    if (ready) {\n        run();\n    }\n}\n",
    );
}

#[test]
fn a_comment_leading_an_access_label_line_leaves_the_label_at_its_column() {
    let input = "class Item {\npublic:\n    int value;\n    Item() : value(0) {}\n    /* note */ private:\n    int other;\n};\n";
    check(
        input,
        &["--style=google"],
        "class Item {\n  public:\n    int value;\n    Item() : value(0) {}\n  /* note */ private:\n    int other;\n};\n",
    );
    check(
        input,
        &["--style=mozilla"],
        "class Item\n{\npublic:\n    int value;\n    Item() : value(0) {}\n/* note */ private:\n    int other;\n};\n",
    );
}

#[test]
fn a_case_block_brace_attaches_to_a_comment_led_label_as_to_any() {
    let input = "void f()\n{\n    switch (kind) {\n    /* note */ case 1: // first\n    {\n        value=0;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (kind) {\n    /* note */ case 1: { // first\n        value=0;\n    }\n    }\n}\n",
    );
}

#[test]
fn indenting_define_bodies_leaves_a_case_block_after_a_conditional_a_case_block() {
    let input = "void f()\n{\n    switch (kind) {\n#ifdef FEATURE\n    case 3:\n#endif\n    {   // own\n        break;\n    }\n    }\n}\n";
    let expected = "void f()\n{\n    switch (kind) {\n#ifdef FEATURE\n    case 3:\n#endif\n    {\n        // own\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=mozilla"], expected);
    check(
        input,
        &["--style=mozilla", "--indent-preproc-define"],
        expected,
    );
}

#[test]
fn a_message_closer_alone_on_its_line_stands_at_its_bracket() {
    let input = "void f()\n{\n    return [self.other countWith:1\n                         and:2\n           ];\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    return [self.other countWith:1\n                       and:2\n           ];\n}\n",
    );
}

#[test]
fn a_comment_leading_the_body_of_a_header_after_else_stands_in_that_body() {
    check(
        "void f()\n{\n    if (ready) {\n        value=1;\n    } else while (other) {\n        /* note */ value=0;\n    }\n}\n",
        &["--style=kr"],
        "void f()\n{\n    if (ready) {\n        value=1;\n    } else while (other) {\n            /* note */ value=0;\n        }\n}\n",
    );
    check(
        "void f()\n{\n    if (ready) {\n        value=1;\n    } else for (;;) {\n        /* note */\n        value=0;\n    }\n}\n",
        &["--style=kr"],
        "void f()\n{\n    if (ready) {\n        value=1;\n    } else for (;;) {\n            /* note */\n            value=0;\n        }\n}\n",
    );
}

#[test]
fn a_comment_between_base_class_rows_stands_at_the_rows() {
    for comment in ["// note", "/* note */"] {
        check(
            &format!(
                "class Derived : public Base,\n                {comment}\n                private Other {{\n}};\n"
            ),
            &["--style=kr"],
            &format!("class Derived : public Base,\n    {comment}\n    private Other\n{{\n}};\n"),
        );
        check(
            &format!(
                "class Derived\n    : public Base,\n  {comment}\n      private Other {{\n}};\n"
            ),
            &["--style=whitesmith"],
            &format!(
                "class Derived\n    : public Base,\n      {comment}\n      private Other\n    {{\n    }};\n"
            ),
        );
    }
}

#[test]
fn a_comment_before_the_body_of_a_class_with_bases_stands_at_the_class() {
    for comment in ["// note", "/* note */"] {
        check(
            &format!(
                "class Derived :\n    public Base,\n    private Other\n    {comment}\n{{\n}};\nstruct Item : Base\n  {comment}\n{{\n}};\n"
            ),
            &["--style=allman"],
            &format!(
                "class Derived :\n    public Base,\n    private Other\n{comment}\n{{\n}};\nstruct Item : Base\n{comment}\n{{\n}};\n"
            ),
        );
    }
}

#[test]
fn a_stream_chain_a_comment_leads_aligns_at_its_operator() {
    check(
        "void f()\n{\n    /* note */ std::cout << first\n              << second;\n    /* note */ std::cout << \"first\"\n              \"second\";\n}\n",
        &["--style=kr"],
        "void f()\n{\n    /* note */ std::cout << first\n                         << second;\n    /* note */ std::cout << \"first\"\n                         \"second\";\n}\n",
    );
}

#[test]
fn a_comment_after_a_broken_off_brace_keeps_its_column_past_another_comment() {
    check(
        "void f()\n{\n    if (ready) {\n        first();\n    } /* c */ else { /* note */\n        result=0;\n    }\n}\n",
        &["--style=allman"],
        "void f()\n{\n    if (ready)\n    {\n        first();\n    } /* c */ else   /* note */\n    {\n        result=0;\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_an_opening_brace_leads_what_follows_it_on_its_line() {
    let input = "void f() { /* a */ value = 1;\n    first();\n}\nvoid g()\n{\n    while (ready) { /* a */ /* b */\n        first();\n    }\n    do { /* a */ value = 1;\n    } while (ready);\n    if (ready) { /* a */ // b\n        first();\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    /* a */ value = 1;\n    first();\n}\nvoid g()\n{\n    while (ready) {\n        /* a */ /* b */\n        first();\n    }\n    do {\n        /* a */ value = 1;\n    } while (ready);\n    if (ready) { /* a */ // b\n        first();\n    }\n}\n",
    );
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    /* a */ value = 1;\n    first();\n}\nvoid g()\n{\n    while (ready)\n    {\n        /* a */ /* b */\n        first();\n    }\n    do\n    {\n        /* a */ value = 1;\n    }\n    while (ready);\n    if (ready) { /* a */ // b\n        first();\n    }\n}\n",
    );
}

#[test]
fn elements_after_a_comment_leading_the_first_align_to_the_first() {
    check(
        "enum Kind { /* a */ First,\n    Second };\nint list[] = { /* a */ 1,\n    2 };\n",
        &["--style=kr"],
        "enum Kind { /* a */ First,\n                    Second\n          };\nint list[] = { /* a */ 1,\n                       2\n             };\n",
    );
}

#[test]
fn a_comment_leading_a_definition_head_leaves_its_broken_brace_alone() {
    check(
        "/* note */ void f()\n/* note */ {\n    value = 1;\n}\n",
        &["--style=kr"],
        "/* note */ void f()\n/* note */\n{\n    value = 1;\n}\n",
    );
}

#[test]
fn a_case_label_continued_past_a_comment_or_directive_stays_a_case_label() {
    check(
        "void f()\n{\n    switch (flags) {\n    case FIRST |\n         // note\n         SECOND:\n        value = 1;\n        break;\n    case FIRST |\n         /* note */\n         THIRD:\n        break;\n    case FIRST |\n#ifdef FEATURE\n         FOURTH:\n#endif\n        break;\n    }\n}\n",
        &["--style=kr"],
        "void f()\n{\n    switch (flags) {\n    case FIRST |\n            // note\n            SECOND:\n        value = 1;\n        break;\n    case FIRST |\n            /* note */\n            THIRD:\n        break;\n    case FIRST |\n#ifdef FEATURE\n            FOURTH:\n#endif\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_comment_leading_a_constructor_head_keeps_its_initializer_spacing() {
    check(
        "/* note */ Item::Item(int value, int other)\n    : value_(value),\n      other_(other)\n{\n}\n",
        &["--style=kr"],
        "/* note */ Item::Item(int value, int other)\n    : value_(value),\n      other_(other)\n{\n}\n",
    );
}

#[test]
fn a_comment_among_call_arguments_after_a_nested_call_stands_at_the_arguments() {
    check(
        "void f()\n{\n    observe(items.begin(), notice,\n            // note\n            other);\n    observe(items.begin(), notice,\n            // note\n            [](int x) {\n        call(x);\n    });\n}\n",
        &["--style=kr"],
        "void f()\n{\n    observe(items.begin(), notice,\n            // note\n            other);\n    observe(items.begin(), notice,\n            // note\n    [](int x) {\n        call(x);\n    });\n}\n",
    );
}

#[test]
fn a_comment_after_a_parameter_declaration_body_brace_opens_the_body() {
    check(
        "int helper(value)\nint value;\n{ /* note */\n    return value;\n}\n",
        &["--style=kr"],
        "int helper(value)\nint value;\n{\n    /* note */\n    return value;\n}\n",
    );
}

#[test]
fn an_enum_opening_the_file_breaks_its_brace_as_anywhere() {
    check(
        "enum Kind {\n    First,\n    Second\n};\nenum Other {\n    Third\n};\n",
        &["--style=allman"],
        "enum Kind\n{\n    First,\n    Second\n};\nenum Other\n{\n    Third\n};\n",
    );
}

#[test]
fn a_line_a_backslash_continues_keeps_its_break() {
    check(
        "void f()\n{\n    if (ready) \\\n        fail();\n    else \\\n        done();\n}\n",
        &["--style=kr"],
        "void f()\n{\n    if (ready) \\\n        fail();\n    else \\\n        done();\n}\n",
    );
}

#[test]
fn a_bare_block_run_in_on_its_own_row_stays_off_the_body_brace() {
    check(
        "void g()\n{\n    {   value = 1; } }\n",
        &["--style=pico"],
        "void g()\n{\n    {   value = 1; } }\n",
    );
    // An aggregate's brace runs into a nested one past comment rows.
    check(
        "int table[][2] =\n{\n    /* first */\n    {\n        1, 2\n    }\n};\n",
        &["--style=pico"],
        "int table[][2] =\n{   /* first */\n    {   1, 2 } };\n",
    );
}

#[test]
fn a_comment_leading_a_line_starts_no_operator_chain() {
    check(
        "void f()\n{\n    /* note */ out << helper(first, (second\n                         << third));\n}\n",
        &["--style=allman"],
        "void f()\n{\n    /* note */ out << helper(first, (second\n                                     << third));\n}\n",
    );
}

#[test]
fn a_comment_leading_the_body_brace_of_a_class_with_bases_stands_at_the_class() {
    check(
        "class Next : public Base\n    /* note */ {\n};\n",
        &["--style=allman"],
        "class Next : public Base\n/* note */\n{\n};\n",
    );
    check(
        "class Next : public Base\n    /* note */ {\n};\n",
        &["--style=java"],
        "class Next : public Base\n/* note */ {\n};\n",
    );
}

#[test]
fn a_comment_led_row_closing_a_paren_stands_where_the_row_would() {
    check(
        "void f()\n{\n    mask = 1 << (((value & (size - 1))\n                  /* note */ >> shift)\n                 + offset);\n}\n",
        &["--style=kr"],
        "void f()\n{\n    mask = 1 << (((value & (size - 1))\n                  /* note */ >> shift)\n                 + offset);\n}\n",
    );
}

#[test]
fn a_comment_leading_a_declarator_continues_no_operator_into_its_brace() {
    check(
        "static const struct Item\n    /* note */ first = {\n        1, 2\n    },\n    second = {\n        3, 4\n    };\n",
        &["--style=vtk"],
        "static const struct Item\n    /* note */ first =\n{\n    1, 2\n},\nsecond =\n{\n    3, 4\n};\n",
    );
}

#[test]
fn an_access_label_a_comment_trails_keeps_its_class_brace_alone() {
    check(
        "class Item\n{\npublic: /* note */\n    int value;\n};\n",
        &["--style=pico"],
        "class Item\n{\npublic: /* note */\n    int value; };\n",
    );
}

#[test]
fn statements_a_comment_leads_keep_the_gap_before_their_trailing_comment() {
    check(
        "void f()\n{\n    /* note */ value=0; result=1;          // both\n}\n",
        &["--style=linux", "--keep-one-line-statements"],
        "void f()\n{\n    /* note */ value=0; result=1;          // both\n}\n",
    );
}

#[test]
fn a_comment_before_a_broken_initializer_brace_stands_at_the_statement() {
    let input = "void f()\n{\n    struct Item item =\n        // note\n    {\n        1, 2\n    };\n    struct Item other =\n        /* note */\n    {\n        1, 2\n    };\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    struct Item item =\n    // note\n    {\n        1, 2\n    };\n    struct Item other =\n    /* note */\n    {\n        1, 2\n    };\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    struct Item item =\n    // note\n        {\n        1, 2\n        };\n    struct Item other =\n    /* note */\n        {\n        1, 2\n        };\n    }\n",
    );
    // GNU's broken brace leaves the comment at the statement as well.
    check(
        input,
        &["--style=gnu"],
        "void f()\n{\n    struct Item item =\n    // note\n    {\n        1, 2\n    };\n    struct Item other =\n    /* note */\n    {\n        1, 2\n    };\n}\n",
    );
}

#[test]
fn a_comment_before_a_nested_braceless_else_stands_at_the_else() {
    check(
        "void f()\n{\n    if (ready)\n        if (other)\n            value=0;\n    // note\n        else\n            result=0;\n}\n",
        &["--style=kr"],
        "void f()\n{\n    if (ready)\n        if (other)\n            value=0;\n        // note\n        else\n            result=0;\n}\n",
    );
}

#[test]
fn a_label_a_comment_leads_after_a_directive_stands_at_the_label_column() {
    check(
        "void f()\n{\n    switch (state) {\n#ifdef X\n#endif\n    /* note */ retry:\n    case 1:\n        value = 1;\n    }\n}\n",
        &["--style=kr"],
        "void f()\n{\n    switch (state) {\n#ifdef X\n#endif\n/* note */ retry:\n    case 1:\n        value = 1;\n    }\n}\n",
    );
}

#[test]
fn base_class_rows_after_the_first_stand_at_its_column() {
    check(
        "struct Item\n    : public Base,\n      private Middle,\n      private Other {\n};\n",
        &["--style=kr"],
        "struct Item\n    : public Base,\n      private Middle,\n      private Other {\n};\n",
    );
}

#[test]
fn a_base_class_row_a_comment_leads_stands_at_the_rows() {
    let input = "struct Item\n    : public Base,\n      /* c */ private Middle,\n      private Other {\n};\n";
    check(
        input,
        &["--style=kr"],
        "struct Item\n    : public Base,\n      /* c */ private Middle,\n      private Other {\n};\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "struct Item\n    : public Base,\n      /* c */ private Middle,\n      private Other\n    {\n    };\n",
    );
    // The brace broken off the row stands at the class.
    check(
        "class Derived\n    : public Base,\n      /* c */ private Other {\n};\n",
        &["--style=kr"],
        "class Derived\n    : public Base,\n      /* c */ private Other\n{\n};\n",
    );
}

#[test]
fn a_class_body_after_rows_of_bases_stands_a_level_past_the_head() {
    check(
        "class Derived : public Base,\n                private Other {\npublic:\n    /* note */\n    int value;\n};\n",
        &["--style=stroustrup"],
        "class Derived : public Base,\n    private Other {\npublic:\n    /* note */\n    int value;\n};\n",
    );
}

#[test]
fn a_declaration_a_comment_leads_keeps_its_pointer_declarator() {
    check(
        "void f()\n{\n    /* note */ Item *item = new Item(value);\n    /* note */ int *list = other;\n}\n",
        &["--pad-oper"],
        "void f()\n{\n    /* note */ Item *item = new Item(value);\n    /* note */ int *list = other;\n}\n",
    );
}

#[test]
fn class_members_after_a_conditional_opening_the_body_stand_in_the_body() {
    check(
        "class Pair {\n#if A\n    int x;\n#endif\n    int y;\n};\n",
        &["--style=whitesmith"],
        "class Pair\n    {\n#if A\n        int x;\n#endif\n        int y;\n    };\n",
    );
}

#[test]
fn a_case_label_a_comment_leads_continues_as_any() {
    check(
        "void f()\n{\n    switch (flags) {\n    /* note */ case FIRST |\n        THIRD:\n        value = 2;\n        break;\n    }\n}\n",
        &["--style=mozilla"],
        "void f()\n{\n    switch (flags) {\n    /* note */ case FIRST |\n            THIRD:\n        value = 2;\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_body_closer_after_a_trailing_comment_stands_at_its_brace_past_a_conditional_head() {
    check(
        "void f()\n#ifdef FEATURE\n{\n#endif\n    count = 1; // note\n}\n",
        &["--style=whitesmith"],
        "void f()\n#ifdef FEATURE\n    {\n#endif\n    count = 1; // note\n    }\n",
    );
}

#[test]
fn an_else_a_comment_leads_stands_at_its_if() {
    check(
        "void f()\n{\n    if (ready)\n        if (other)\n#ifdef FEATURE\n            { value=0; }\n#endif\n        /* note */ else\n            { result=0; }\n}\n",
        &["--style=1tbs"],
        "void f()\n{\n    if (ready)\n        if (other)\n#ifdef FEATURE\n        {\n            value=0;\n        }\n#endif\n        /* note */ else {\n            result=0;\n        }\n}\n",
    );
}

#[test]
fn a_comment_before_the_first_row_of_a_nested_element_stands_at_the_row() {
    check(
        "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, {\n            // note\n            .value = 2,\n        }\n    };\n}\n",
        &["--style=kr"],
        "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, {\n            // note\n            .value = 2,\n        }\n    };\n}\n",
    );
}

#[test]
fn a_declarator_split_from_its_type_continues_past_a_comment_line() {
    check(
        "void f()\n{\n    static const struct Item\n        // note\n        first = {\n        1, 2\n    };\n}\n",
        &["--style=linux"],
        "void f()\n{\n    static const struct Item\n        // note\n        first = {\n        1, 2\n    };\n}\n",
    );
}

#[test]
fn elements_of_a_list_a_comment_leads_align_to_its_first_element() {
    check(
        "void f()\n{\n    /* note */ int list[] = { 1,\n        2\n    };\n}\n",
        &["--style=kr"],
        "void f()\n{\n    /* note */ int list[] = { 1,\n                              2\n                            };\n}\n",
    );
}

#[test]
fn a_list_a_comment_leads_keeps_its_first_element_on_the_brace_line() {
    check(
        "void f()\n{\n    /* note */ int list[] = { 1,\n        2\n    };\n}\n",
        &["--style=allman"],
        "void f()\n{\n    /* note */ int list[] = { 1,\n                              2\n                            };\n}\n",
    );
}

#[test]
fn a_header_body_line_a_comment_leads_is_no_comment_line_for_the_row_after() {
    check(
        "int table[] = {\n    1,\n};\nvoid f()\n{\n    if (ready) {\n        /* note */ int list[] = { 1,\n                       2 };\n    }\n}\n",
        &["--style=google"],
        "int table[] = {\n    1,\n};\nvoid f() {\n    if (ready) {\n        /* note */ int list[] = { 1,\n                                  2\n                                };\n    }\n}\n",
    );
}

#[test]
fn a_head_in_the_other_branch_after_a_brace_stays_a_head() {
    check(
        "#ifdef WIDE\nint wide_main(int argc) {\n#else\nint main(int argc) {\n#endif\n    return 0;\n}\n",
        &["--style=kr", "--break-return-type"],
        "#ifdef WIDE\nint\nwide_main(int argc)\n{\n#else\nint\nmain(int argc)\n{\n#endif\n    return 0;\n}\n",
    );
}

#[test]
fn a_case_block_closer_after_a_body_line_a_comment_leads_keeps_the_block_column() {
    check(
        "void f()\n{\n    switch (state) {\n    case 1: {\n        /* note */ first();\n    }\n    break;\n    }\n}\n",
        &["--style=stroustrup", "--indent-cases"],
        "void f()\n{\n    switch (state) {\n    case 1: {\n            /* note */ first();\n        }\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_comment_led_line_in_a_ratliff_block_stands_at_the_block_brace_column() {
    let input = "void f()\n{\n    {\n#ifdef FEATURE\n    retry:\n#endif\n        /* note */ value = 1;\n    }\n}\n";
    let expected = "void f() {\n        {\n#ifdef FEATURE\nretry:\n#endif\n        /* note */ value = 1;\n        }\n    }\n";
    check(input, &["--style=ratliff"], expected);
    check(input, &["--style=banner"], expected);
}

#[test]
fn a_ratliff_closer_after_alternative_braces_of_a_header_stands_at_the_body() {
    check(
        "void f()\n{\n    if (ready)\n#ifdef A\n    {\n#else\n    {\n#endif\n        value = 1;\n    }\n}\n",
        &["--style=ratliff"],
        "void f() {\n    if (ready)\n#ifdef A\n        {\n#else\n        {\n#endif\n        value = 1;\n        }\n    }\n",
    );
}

#[test]
fn a_header_after_directives_after_a_comment_led_brace_line_breaks_its_block() {
    check(
        "/* note */ void f()\n{\n#ifdef FEATURE\n    if (ready) {\n        value = 0;\n    }\n#endif\n}\n",
        &["--style=java", "--break-blocks=all"],
        "/* note */ void f() {\n#ifdef FEATURE\n\n    if (ready) {\n        value = 0;\n    }\n\n#endif\n}\n",
    );
    check(
        "/* first\n   second */ void f()\n{\n#ifdef FEATURE\n    if (ready) {\n        value = 0;\n    }\n#endif\n}\n",
        &["--style=java", "--break-blocks=all"],
        "/* first\n   second */ void f() {\n#ifdef FEATURE\n\n    if (ready) {\n        value = 0;\n    }\n\n#endif\n}\n",
    );
}

#[test]
fn a_header_after_code_closing_a_block_comment_breaks_its_block() {
    check(
        "void f()\n{\n    value = 0;\n    /* first\n       second */ value = 1;\n    if (ready) {\n        value = 0;\n    }\n}\n",
        &["--style=java", "--break-blocks"],
        "void f() {\n    value = 0;\n    /* first\n       second */ value = 1;\n\n    if (ready) {\n        value = 0;\n    }\n}\n",
    );
}

#[test]
fn a_statement_broken_off_a_block_closer_keeps_the_gap_before_its_comment() {
    check(
        "void f()\n{\n    if (ready) {\n        value=0;\n    } value=1;    /* note */\n}\n",
        &["--style=kr"],
        "void f()\n{\n    if (ready) {\n        value=0;\n    }\n    value=1;    /* note */\n}\n",
    );
    check(
        "void f()\n{\n    if (ready) {\n        value=0;\n    } value=1;    /* note */\n}\n",
        &["--style=kr", "--pad-oper"],
        "void f()\n{\n    if (ready) {\n        value = 0;\n    }\n    value = 1;  /* note */\n}\n",
    );
    check(
        "void f()\n{\n    while (ready) {\n        value=0;\n    } other(1); // note\n}\n",
        &["--style=gnu"],
        "void f()\n{\n    while (ready)\n        {\n            value=0;\n        }\n    other(1); // note\n}\n",
    );
}

#[test]
fn an_interface_line_a_comment_leads_sets_no_continuation_for_its_members() {
    let input = "/* note */ @interface Item : NSObject\n@property (nonatomic) int value;\n- (void)update:(int)value;\n@end\n";
    check(input, &["--style=kr"], input);
    check(input, &["--style=vtk"], input);
}

#[test]
fn a_message_argument_after_a_trailing_block_comment_aligns_its_colon() {
    let input = "- (void)update:(int)value {\n    [self apply:value /* note */\n          with:other];\n}\n";
    check(input, &["--style=java"], input);
}

#[test]
fn a_comment_before_a_nested_element_closer_stands_at_its_rows() {
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n            // note\n        }, {\n            .value = 2,\n            /* note */\n        }\n    };\n}\n";
    check(input, &["--style=kr"], input);
    check(input, &["--style=mozilla"], input);
    let after_label = "void f()\n{\n    switch (kind) {\n    case 1:\n        int list[] = { {\n                1,\n                // note\n            }\n        };\n    }\n}\n";
    check(after_label, &["--style=kr"], after_label);
}

#[test]
fn a_comment_before_the_closer_of_a_list_of_nested_elements_stands_at_its_body() {
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, {\n            .value = 2,\n        }\n        // note\n    };\n    int table[][2] = { { 1, 2 },\n        { 3, 4 },\n        // note\n    };\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_lambda_argument_past_a_directive_stands_as_one_past_a_comma() {
    check(
        "void f()\n{\n    std::sort(items.begin(), items.end(),\n#ifdef FEATURE\n              [](const Item &first, const Item &second) {\n#endif\n        return first.value < second.value;\n    });\n}\n",
        &["--style=kr"],
        "void f()\n{\n    std::sort(items.begin(), items.end(),\n#ifdef FEATURE\n    [](const Item &first, const Item &second) {\n#endif\n        return first.value < second.value;\n    });\n}\n",
    );
}

#[test]
fn a_header_after_a_brace_a_comment_leads_past_a_directive_gets_no_blank_line() {
    let input = "void f()\n{\n#if FEATURE\n    if( ready ) {\n        run();\n    } else\n#endif\n        /*if( other )*/{\n        if( value==0 ) {\n            run();\n        }\n    }\n}\n";
    check(
        input,
        &["--style=java", "--break-blocks"],
        "void f() {\n#if FEATURE\n\n    if( ready ) {\n        run();\n    } else\n#endif\n        /*if( other )*/{\n        if( value==0 ) {\n            run();\n        }\n    }\n}\n",
    );
}

#[test]
fn a_comment_before_a_compound_literal_closer_stands_at_its_body() {
    let input = "void f()\n{\n    value = (struct Item) {\n        .value = 1,\n        .other = 2\n        // note\n    };\n    call((struct Item) {\n        .other = 2\n        // note\n    });\n    return (struct Item) {\n        1,\n        2\n        // note\n    };\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_lambda_argument_a_comment_leads_stands_as_one_past_a_comma() {
    let input = "void f()\n{\n    std::sort(items.begin(), items.end(),\n              /* note */ [](const Item &first, const Item &second) {\n        return first.value < second.value;\n    });\n}\n";
    check(
        input,
        &["--style=linux"],
        "void f()\n{\n    std::sort(items.begin(), items.end(),\n    /* note */ [](const Item &first, const Item &second) {\n        return first.value < second.value;\n    });\n}\n",
    );
    let past_directive = "void f()\n{\n#ifdef FEATURE\n    std::sort(items.begin(), items.end(),\n#endif\n              /* note */ [](const Item &first, const Item &second) {\n                  return first.value < second.value;\n              });\n}\n";
    check(
        past_directive,
        &["--style=linux"],
        "void f()\n{\n#ifdef FEATURE\n    std::sort(items.begin(), items.end(),\n#endif\n    /* note */ [](const Item &first, const Item &second) {\n        return first.value < second.value;\n    });\n}\n",
    );
}

#[test]
fn a_comment_after_a_brace_broken_off_a_line_a_comment_leads_keeps_its_gap() {
    let input = "void f()\n{\n    if (ready) {\n        value=0;\n    /* note */ } else if (other) { // second\n        value=1;\n    }\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    if (ready)\n    {\n        value=0;\n        /* note */\n    }\n    else if (other)     // second\n    {\n        value=1;\n    }\n}\n",
    );
    check(
        "void f()\n{\n    if (ready) {\n        value=0;\n    /* note */ } value = 1; // second\n}\n",
        &["--style=kr"],
        "void f()\n{\n    if (ready) {\n        value=0;\n        /* note */\n    }\n    value = 1; // second\n}\n",
    );
}

#[test]
fn a_brace_row_after_a_brace_row_a_comment_leads_stands_at_the_brace_rows() {
    let input = "void f()\n{\n    struct Item items[] = {\n        /* note */ { .value = 1, .name = \"a\" },\n        { .value = 2, .name = \"b\" },\n    };\n}\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n    struct Item items[] = {\n        /* note */ { .value = 1, .name = \"a\" },\n            { .value = 2, .name = \"b\" },\n        };\n    }\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    struct Item items[] =\n        {\n        /* note */ { .value = 1, .name = \"a\" },\n            { .value = 2, .name = \"b\" },\n        };\n    }\n",
    );
}

#[test]
fn a_nested_element_a_comment_leads_closes_at_its_row_in_a_case_block() {
    let input = "void f()\n{\n    switch (op) {\n    default: {\n        int table[][2] = {\n            { FIRST, 1 },\n            /* note */ {\n                SECOND, 2\n            },\n        };\n        break;\n    }\n    }\n}\n";
    check(input, &[], input);
}

#[test]
fn a_vtk_else_block_broken_past_a_directive_indents_its_brace() {
    check(
        "void f()\n{\n    if (other)\n        { value=0; }\n    else\n#ifdef FEATURE\n        { result=0; }\n#endif\n}\n",
        &["--style=vtk"],
        "void f()\n{\n    if (other)\n        {\n        value=0;\n        }\n    else\n#ifdef FEATURE\n        {\n        result=0;\n        }\n#endif\n}\n",
    );
}

#[test]
fn a_statement_a_comment_leads_after_a_label_block_in_a_case_stands_in_the_body() {
    check(
        "void f()\n{\n    switch (flags) {\n    case FIRST: {\n    retry: {\n            value = 1;\n            goto retry;\n        }\n        /* note */ break;\n    }\n    }\n}\n",
        &["--style=kr"],
        "void f()\n{\n    switch (flags) {\n    case FIRST: {\nretry: {\n            value = 1;\n            goto retry;\n        }\n        /* note */ break;\n    }\n    }\n}\n",
    );
    check(
        "void f()\n{\n    switch (flags) {\n    case FIRST: {\n    retry: {\n            value = 1;\n            goto retry;\n        }\n        /* note */ break;\n    }\n    }\n}\n",
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (flags)\n        {\n        case FIRST:\n            {\nretry:\n                {\n                value = 1;\n                goto retry;\n                }\n            /* note */ break;\n            }\n        }\n    }\n",
    );
}

#[test]
fn a_comment_split_off_a_closer_after_a_bare_block_stands_at_the_body() {
    let input = "void f()\n{\n    { value=0; }\n    /* note */ }\n";
    check(
        input,
        &["--style=ratliff"],
        "void f() {\n        {\n        value=0;\n        }\n    /* note */\n    }\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n        {\n        value=0;\n        }\n    /* note */\n    }\n",
    );
}

#[test]
fn a_declarator_split_from_an_aggregate_type_a_comment_leads_continues() {
    let input = "void f()\n{\n    /* note */ static const struct Item\n        first = 1;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_statement_a_comment_leads_after_a_run_in_case_block_closer_stands_as_a_comment_line() {
    check(
        "void f()\n{\n    switch (kind) {\n    case 1: {\n        value=0; }\n    /* note */\n    break;\n    case 2: {\n        value=0; }\n    /* note */ break; }\n}\n",
        &["--style=lisp"],
        "void f() {\n    switch (kind) {\n    case 1: {\n        value=0; }\n        /* note */\n    break;\n    case 2: {\n        value=0; }\n        /* note */ break; } }\n",
    );
}

#[test]
fn an_indented_conditional_closes_at_its_opening_in_a_case_body() {
    let input = "void f()\n{\n    switch (k) {\n    case 2:\n        int other[] = {\n            1, 2\n#ifdef FEATURE\n        };\n#endif\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=linux", "--indent-preproc-cond"],
        "void f()\n{\n    switch (k) {\n    case 2:\n        int other[] = {\n            1, 2\n            #ifdef FEATURE\n        };\n            #endif\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_continuation_a_comment_leads_after_a_head_a_comment_leads_aligns_with_its_value() {
    let input = "void f()\n{\n    /* note */ return first ||\n              /* c */ second;\n    /* note */ value = first &&\n        /* c */ second;\n}\n";
    check(
        input,
        &["--style=java"],
        "void f() {\n    /* note */ return first ||\n                      /* c */ second;\n    /* note */ value = first &&\n                       /* c */ second;\n}\n",
    );
}

#[test]
fn a_comment_in_an_empty_switch_body_stands_past_its_labels() {
    let input =
        "void f()\n{\n    switch (op) {\n    /* c */\n    }\n    switch (op) {\n    /* c */ }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (op) {\n        /* c */\n    }\n    switch (op) {\n        /* c */\n    }\n}\n",
    );
    check(
        input,
        &["--style=allman", "--indent-switches"],
        "void f()\n{\n    switch (op)\n    {\n            /* c */\n    }\n    switch (op)\n    {\n            /* c */\n    }\n}\n",
    );
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (op)\n        {\n            /* c */\n        }\n    switch (op)\n        {\n            /* c */\n        }\n    }\n",
    );
}

#[test]
fn an_if_constexpr_else_split_by_a_directive_closes_at_its_block() {
    let input = "void f()\n{\n    if constexpr (sizeof(Item) > 4) {\n        value = 1;\n#ifdef FEATURE\n    } else {\n#endif\n        value = 2;\n    }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    if constexpr (sizeof(Item) > 4)\n        {\n        value = 1;\n#ifdef FEATURE\n        }\n    else\n        {\n#endif\n        value = 2;\n        }\n    }\n",
    );
}

#[test]
fn a_case_label_continued_past_a_trailing_block_comment_continues() {
    let input = "void f()\n{\n    switch (flags) {\n    case FIRST | /* note */\n        SECOND:\n        value = 1;\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (flags) {\n    case FIRST | /* note */\n            SECOND:\n        value = 1;\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_macro_header_a_comment_leads_indents_its_body() {
    check(
        "#define CHECK(A) \\\n      do {           \\\n        /* note */ if (A)       \\\n          fail();    \\\n      } while (0)\n",
        &["--style=kr", "--indent-preproc-define"],
        "#define CHECK(A) \\\n    do {           \\\n        /* note */ if (A)       \\\n            fail();    \\\n    } while (0)\n",
    );
    check(
        "#define CHECK(A) \\\n      switch (A) {           \\\n        /* note */ case 1:       \\\n          fail();    \\\n          break;    \\\n      }\n",
        &["--style=kr", "--indent-preproc-define"],
        "#define CHECK(A) \\\n    switch (A) {           \\\n    /* note */ case 1:       \\\n        fail();    \\\n        break;    \\\n    }\n",
    );
}

#[test]
fn a_case_label_a_comment_leads_keeps_the_gap_before_its_brace_comment() {
    let input = "void f()\n{\n    switch (kind) {\n    /* note */ case 1: {    // note\n        value=0;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_member_after_a_directive_in_a_case_block_type_stands_at_the_members() {
    let input = "void f()\n{\n    switch (state) {\n    case 1: {\n#ifdef FEATURE\n        static const struct {\n#endif\n            int value;\n        } items[] = {\n            { 1 },\n        };\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    switch (state)\n    {\n    case 1:\n    {\n#ifdef FEATURE\n        static const struct\n        {\n#endif\n            int value;\n        } items[] =\n        {\n            { 1 },\n        };\n        break;\n    }\n    }\n}\n",
    );
}

#[test]
fn a_closer_after_a_directive_of_a_brace_a_comment_leads_stands_at_the_header() {
    let input = "int f(void)\n{\n    for (i = 0; i < n; i++)\n    /* c */ {\n        g();\n#if A\n        h();\n#endif\n    }\n    return 0;\n}\n";
    check(
        input,
        &["--style=kr"],
        "int f(void)\n{\n    for (i = 0; i < n; i++)\n        /* c */ {\n        g();\n#if A\n        h();\n#endif\n    }\n    return 0;\n}\n",
    );
}

#[test]
fn an_argument_after_a_comment_line_in_a_call_a_comment_leads_aligns_with_the_paren() {
    let input = "void f()\n{\n    /* c */ std::sort(items.begin(), items.end(),\n              // note\n              [](const Item &first, const Item &second) {\n        return first.value < second.value;\n    });\n}\n";
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    /* c */ std::sort(items.begin(), items.end(),\n                      // note\n                      [](const Item &first, const Item &second)\n    {\n        return first.value < second.value;\n    });\n}\n",
    );
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    /* c */ std::sort(items.begin(), items.end(),\n                      // note\n    [](const Item &first, const Item &second) {\n        return first.value < second.value;\n    });\n}\n",
    );
}

#[test]
fn a_comment_after_a_nested_switch_in_a_case_stands_at_the_switch() {
    let input = "void f()\n{\n    switch (kind)\n    {\n    case 1:\n        switch (op)\n        {\n        }\n    /* c */ }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (kind)\n        {\n        case 1:\n            switch (op)\n                {\n                }\n            /* c */\n        }\n    }\n",
    );
}

#[test]
fn a_switch_brace_a_comment_leads_stays_off_the_header_as_any_header_brace() {
    let input =
        "void f(void)\n{\n    switch (op)\n    /* c */ {\n    case A:\n        break;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f(void)\n{\n    switch (op)\n        /* c */ {\n    case A:\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_comment_after_code_broken_off_a_leading_comment_keeps_its_gap() {
    check(
        "struct S {\n    int x;\n/* c */ } s, t;/* e */\n",
        &["--style=kr"],
        "struct S {\n    int x;\n    /* c */\n} s, t;/* e */\n",
    );
}

#[test]
fn a_template_parameter_a_comment_leads_aligns_with_the_parameters() {
    let input = "template <typename First,\n          /* c */ typename Second>\nclass Other {};\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_subscript_continued_past_a_line_comment_keeps_its_column() {
    let input = "void f()\n{\n    value = table[first +\n                  // note\n                  second];\n    if (table[first +\n              // note\n              second] && other) {\n        value = 0;\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn an_initializer_a_comment_line_parts_from_its_assignment_stays_on_one_line() {
    let input = "void f()\n{\n    struct Item other =\n        // note\n        { 3, 4 };\n}\n";
    check(
        input,
        &[],
        "void f()\n{\n    struct Item other =\n    // note\n    { 3, 4 };\n}\n",
    );
}

#[test]
fn a_case_label_a_comment_leads_after_a_user_label_stands_at_the_labels() {
    let input = "void f()\n{\n    switch (state) {\n    retry:\n    /* note */ case 1:\n        value = 1;\n        break;\n    case 2:\n        goto retry;\n    }\n}\n";
    check(
        input,
        &["--keep-one-line-statements"],
        "void f()\n{\n    switch (state) {\nretry:\n    /* note */ case 1:\n        value = 1;\n        break;\n    case 2:\n        goto retry;\n    }\n}\n",
    );
}

#[test]
fn comments_before_a_declarator_after_a_closed_initializer_share_its_column() {
    let input = "static const struct Item\n    first = {\n        1, 2\n    },\n    /* note */\n    // note\n    second = {\n        3, 4\n    };\n";
    check(
        input,
        &["--style=whitesmith"],
        "static const struct Item\n    first =\n    {\n    1, 2\n    },\n/* note */\n// note\nsecond =\n    {\n    3, 4\n    };\n",
    );
}

#[test]
fn a_comment_leading_a_case_label_of_a_switch_in_a_case_block_leaves_it_at_the_case_column() {
    let input = "void f()\n{\n    switch (kind) {\n    case 1: {\n        switch (other) {\n        /* note */ case 2:\n            value = 2;\n            break;\n        }\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    switch (kind)\n    {\n    case 1:\n    {\n        switch (other)\n        {\n        /* note */ case 2:\n            value = 2;\n            break;\n        }\n        break;\n    }\n    }\n}\n",
    );
    check(
        input,
        &["--style=gnu"],
        "void f()\n{\n    switch (kind)\n        {\n        case 1:\n        {\n            switch (other)\n                {\n                /* note */ case 2:\n                    value = 2;\n                    break;\n                }\n            break;\n        }\n        }\n}\n",
    );
}

#[test]
fn a_comment_leading_the_first_case_label_of_indented_switch_braces_leaves_it_at_the_case_column() {
    let first = "void f()\n{\n    switch (kind) {\n    /* note */ case 2:\n        value = 2;\n        break;\n    }\n}\n";
    let nested = "void f()\n{\n    switch (kind) {\n    case 1:\n        switch (other) {\n        /* note */ case 2:\n            value = 2;\n            break;\n        }\n        break;\n    }\n}\n";
    check(
        first,
        &["--style=ratliff", "--indent-switches"],
        "void f() {\n    switch (kind) {\n        /* note */ case 2:\n            value = 2;\n            break;\n        }\n    }\n",
    );
    check(
        first,
        &["--style=vtk", "--indent-switches"],
        "void f()\n{\n    switch (kind)\n        {\n        /* note */ case 2:\n            value = 2;\n            break;\n        }\n}\n",
    );
    check(
        nested,
        &["--style=ratliff", "--indent-switches"],
        "void f() {\n    switch (kind) {\n        case 1:\n            switch (other) {\n                /* note */ case 2:\n                    value = 2;\n                    break;\n                }\n            break;\n        }\n    }\n",
    );
    check(
        nested,
        &["--style=vtk", "--indent-switches"],
        "void f()\n{\n    switch (kind)\n        {\n        case 1:\n            switch (other)\n                {\n                /* note */ case 2:\n                    value = 2;\n                    break;\n                }\n            break;\n        }\n}\n",
    );
}

#[test]
fn comments_in_a_template_head_over_lines_leave_its_rows_and_its_end_as_without_them() {
    let expected = "template <typename First,\n          typename Second> /* note */\nclass Pair\n{\n    First first;\n};\n";
    check(expected, &["--style=gnu"], expected);
    let line_comment = expected.replace("/* note */", "// note");
    check(&line_comment, &["--style=gnu"], &line_comment);
    let angle_comment = "template <typename First, /* a < b */\n          typename Second>\nclass Pair\n{\n    First first;\n};\n";
    check(angle_comment, &["--style=gnu"], angle_comment);
}

#[test]
fn a_run_in_closer_of_an_enclosing_list_stays_after_the_comment_leading_its_line() {
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }\n    /* c */ };\n}\n";
    check(
        input,
        &["--style=pico"],
        "void f()\n{   struct Item list[] = { {\n            .value = 1, }\n        /* c */ }; }\n",
    );
    check(
        input,
        &["--style=lisp"],
        "void f() {\n    struct Item list[] = { {\n            .value = 1, }\n        /* c */ }; }\n",
    );
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }\n        /* c */\n    };\n}\n",
    );
}

#[test]
fn a_run_in_closer_of_an_enclosing_list_stays_after_a_comment_trailing_its_row() {
    let input = "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, other /* c */ };\n}\n";
    check(
        input,
        &["--style=pico"],
        "void f()\n{   struct Item list[] = { {\n            .value = 1, }, other /* c */ }; }\n",
    );
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    struct Item list[] = { {\n            .value = 1,\n        }, other /* c */\n    };\n}\n",
    );
}

#[test]
fn adjacent_comments_leading_a_list_closer_stand_at_the_elements() {
    let input = "void f()\n{\n    int list[] = {\n        1,\n    /* c */ /* d */ };\n    struct Item more[] = { {\n            .value = 1,\n        }\n    /* c */ /* d */ };\n}\n";
    check(
        input,
        &["--style=pico"],
        "void f()\n{   int list[] =\n    {   1,\n        /* c */ /* d */ };\n    struct Item more[] = { {\n            .value = 1, }\n        /* c */ /* d */ }; }\n",
    );
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    int list[] = {\n        1,\n        /* c */ /* d */\n    };\n    struct Item more[] = { {\n            .value = 1,\n        }\n        /* c */ /* d */\n    };\n}\n",
    );
}

#[test]
fn adjacent_comments_leading_a_row_leave_it_where_one_comment_does() {
    let input = "void f()\n{\n    value = first +\n        /* a */ /* b */ second;\n    call(first,\n         /* a */ /* b */ second);\n    if (ready)\n        /* a */ /* b */ run();\n    if (ready &&\n        /* a */ /* b */ other)\n        run();\n}\nint x = 1 +\n    /* a */ /* b */ 2;\n";
    let expected = "void f()\n{\n    value = first +\n            /* a */ /* b */ second;\n    call(first,\n         /* a */ /* b */ second);\n    if (ready)\n        /* a */ /* b */ run();\n    if (ready &&\n            /* a */ /* b */ other)\n        run();\n}\nint x = 1 +\n        /* a */ /* b */ 2;\n";
    check(input, &[], expected);
    check(
        &input.replace("/* a */ /* b */", "/* a */"),
        &[],
        &expected.replace("/* a */ /* b */", "/* a */"),
    );
}

#[test]
fn a_message_row_a_comment_leads_stands_at_the_rows_of_its_message() {
    let input = "- (void)run\n{\n    [object performWith:first\n     /* a */ andWith:second];\n    [object performWith:first\n                andWith:second];\n    [object performWith:first\n        /* a */ andWith:second\n        /* b */ orWith:third];\n}\n";
    check(
        input,
        &["--style=kr"],
        "- (void)run\n{\n    [object performWith:first\n            /* a */ andWith:second];\n    [object performWith:first\n            andWith:second];\n    [object performWith:first\n            /* a */ andWith:second\n            /* b */ orWith:third];\n}\n",
    );
    // Aligned colons count the comment among the row's text.
    check(
        input,
        &["--style=kr", "--align-method-colon"],
        "- (void)run\n{\n    [object performWith:first\n        /* a */ andWith:second];\n    [object performWith:first\n                andWith:second];\n    [object performWith:first\n        /* a */ andWith:second\n         /* b */ orWith:third];\n}\n",
    );
}

#[test]
fn a_definition_body_after_a_parameter_row_a_comment_leads_stands_at_the_definition() {
    let input = "static void apply(int (*handler)(int value,\n                                 /* note */ int other), int value)\n{\n    handler(value, 0);\n}\n";
    for style in ["--style=kr", "--style=mozilla", "--style=allman"] {
        check(input, &[style], input);
    }
}

#[test]
fn an_initializer_row_after_a_directive_keeps_its_column_with_the_body_brace_attached() {
    let input = "Item::Item(int value, int other)\n#ifdef FEATURE\n    : value_(value),\n#endif\n      other_(other)\n{\n}\n";
    check(
        input,
        &["--style=java"],
        "Item::Item(int value, int other)\n#ifdef FEATURE\n    : value_(value),\n#endif\n      other_(other) {\n}\n",
    );
    check(
        input,
        &["--style=lisp"],
        "Item::Item(int value, int other)\n#ifdef FEATURE\n    : value_(value),\n#endif\n      other_(other) {}\n",
    );
}

#[test]
fn an_initializer_row_between_directives_keeps_the_column_of_the_rows() {
    let input = "Item::Item(int value, int other)\n    : value_(value),\n#ifdef FEATURE\n      extra_(0),\n#endif\n      other_(other) {\n}\n";
    check(input, &["--style=java"], input);
    check(
        input,
        &["--style=lisp"],
        "Item::Item(int value, int other)\n    : value_(value),\n#ifdef FEATURE\n      extra_(0),\n#endif\n      other_(other) {}\n",
    );
}

#[test]
fn a_header_after_a_directive_takes_its_brace_before_its_comment_as_anywhere() {
    let input = "void f()\n{\n#ifdef FEATURE\n    run();\n#endif\n    if (ready) /* why */ {\n        value=0;\n    }\n    if (ready) /* why */ {\n        value=0;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n#ifdef FEATURE\n    run();\n#endif\n    if (ready) { /* why */\n        value=0;\n    }\n    if (ready) { /* why */\n        value=0;\n    }\n}\n",
    );
}

#[test]
fn a_comment_over_lines_trailing_a_switch_brace_stands_where_one_on_its_own_line_does() {
    // astyle sets it a level deeper on a first pass and moves it on the
    // next; the comment stands at the labels at once.
    let input = "int f(int kind)\n{\n  switch( kind ){  /* Switch on the class of the first byte\n                   ** of the token. */\n    case 1: {\n      return 1;\n    }\n  }\n  return 0;\n}\n";
    let cases: [(&[&str], &str); 4] = [
        (
            &["--style=kr"],
            "int f(int kind)\n{\n    switch( kind ) {\n    /* Switch on the class of the first byte\n                     ** of the token. */\n    case 1: {\n        return 1;\n    }\n    }\n    return 0;\n}\n",
        ),
        (
            &["--style=kr", "--indent-switches"],
            "int f(int kind)\n{\n    switch( kind ) {\n        /* Switch on the class of the first byte\n                         ** of the token. */\n        case 1: {\n            return 1;\n        }\n    }\n    return 0;\n}\n",
        ),
        (
            &["--style=allman"],
            "int f(int kind)\n{\n    switch( kind )\n    {\n    /* Switch on the class of the first byte\n                     ** of the token. */\n    case 1:\n    {\n        return 1;\n    }\n    }\n    return 0;\n}\n",
        ),
        (
            &["--style=gnu"],
            "int f(int kind)\n{\n    switch( kind )\n        {\n        /* Switch on the class of the first byte\n                         ** of the token. */\n        case 1:\n        {\n            return 1;\n        }\n        }\n    return 0;\n}\n",
        ),
    ];
    for (options, expected) in cases {
        check(input, options, expected);
        check(expected, options, expected);
    }
}

#[test]
fn an_empty_block_keeps_the_code_after_it_on_its_line() {
    let input =
        "void f()\n{\n    if (ready) {} else { value = 1; }\n    while (busy()) {} next();\n}\n";
    let attached = "void f()\n{\n    if (ready) {} else {\n        value = 1;\n    }\n    while (busy()) {} next();\n}\n";
    check(input, &[], attached);
    check(input, &["--style=kr"], attached);
    // A style breaking closing headers off their `}` breaks the `else`.
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    if (ready) {}\n    else\n    {\n        value = 1;\n    }\n    while (busy()) {} next();\n}\n",
    );
    check(
        "void f()\n{\n    if (ready) {} // note\n    else { value = 1; }\n    while (busy()) {} /* c */ next();\n    for (;;) {} next();\n    do {} while (busy()); next();\n    if (ready) {} else if (other) {} else {}\n}\n",
        &["--style=kr"],
        "void f()\n{\n    if (ready) {} // note\n    else {\n        value = 1;\n    }\n    while (busy()) {} /* c */ next();\n    for (;;) {} next();\n    do {} while (busy());\n    next();\n    if (ready) {} else if (other) {} else {}\n}\n",
    );
    // A source that parts them keeps them parted.
    let parted = "void f()\n{\n    if (ready) {}\n    else {\n        value = 1;\n    }\n    while (busy()) {}\n    next();\n}\n";
    check(parted, &["--style=kr"], parted);
}

#[test]
fn a_list_row_a_comment_leads_in_a_case_block_stands_at_the_rows() {
    let input = "void f()\n{\n    switch (op) {\n    case 1: {\n        int other[] = { 1,\n                        /* e */ 2\n                      };\n        int table[][2] = {\n            { FIRST, 1 },\n            {\n                SECOND, 2|\n                /* note */ 4\n            },\n        };\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_run_in_switch_brace_before_a_case_block_holding_a_table_stays_at_the_switch() {
    // astyle sets the brace at column one here, though not when the case
    // block holds no table.
    let input = "void f()\n{\n    switch (op) {\n    default: {\n        int table[][2] = {\n            { FIRST, 1 },\n            {\n                SECOND, 2|\n                /* note */ 4\n            },\n        };\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=horstmann"],
        "void f()\n{   switch (op)\n    {   default:\n        {   int table[][2] =\n            {   { FIRST, 1 },\n                {   SECOND, 2|\n                    /* note */ 4\n                },\n            };\n            break;\n        }\n    }\n}\n",
    );
}

#[test]
fn a_brace_after_a_directive_and_a_comment_after_a_case_label_stays_a_plain_block() {
    let input = "void f()\n{\n    switch (state) {\n    case 2:\n#line 46 \"input.l\"\n    {\n        pop(state);\n    }\n    break;\n    case 3:\n#line 46 \"input.l\"\n    // note\n    {\n        pop(state);\n    }\n    break;\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (state) {\n    case 2:\n#line 46 \"input.l\"\n        {\n            pop(state);\n        }\n        break;\n    case 3:\n#line 46 \"input.l\"\n        // note\n        {\n            pop(state);\n        }\n        break;\n    }\n}\n",
    );
}

#[test]
fn a_comment_before_a_directive_ahead_of_a_label_in_a_case_block_stays_in_the_block_body() {
    // As without the directive, the comment stands with the block's
    // statements, not past the label after it.
    let input = "void f()\n{\n    switch (op) {\n    case FIRST: {\n        if (ready) {\n            reset(value);\n        }\n        /* fall through */\n#ifdef FEATURE\n    case SECOND:\n#endif\n        apply(value);\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=kr"],
        "void f()\n{\n    switch (op) {\n    case FIRST: {\n        if (ready) {\n            reset(value);\n        }\n        /* fall through */\n#ifdef FEATURE\n        case SECOND:\n#endif\n            apply(value);\n            break;\n    }\n    }\n}\n",
    );
}

#[test]
fn indented_conditionals_after_a_label_in_a_case_block_stand_at_the_label_body() {
    let input = "void f()\n{\n    switch (op) {\n    case FIRST: {\n        run();\n#ifdef FEATURE\n    case SECOND:\n#endif\n        apply(value);\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--indent-preproc-cond"],
        "void f()\n{\n    switch (op) {\n    case FIRST: {\n        run();\n        #ifdef FEATURE\n        case SECOND:\n            #endif\n            apply(value);\n            break;\n    }\n    }\n}\n",
    );
}

#[test]
fn deleting_empty_lines_keeps_those_of_a_type_body_in_a_function() {
    let input = "void f()\n{\n    if (ready) {\n        struct {\n            int value;\n\n        } item;\n\n        run();\n    }\n}\n";
    check(
        input,
        &["--style=kr", "--delete-empty-lines"],
        "void f()\n{\n    if (ready) {\n        struct {\n            int value;\n\n        } item;\n        run();\n    }\n}\n",
    );
}

#[test]
fn a_call_argument_row_in_a_compound_literal_stays_at_its_paren() {
    let input = "void f()\n{\n    value = (struct Item) {\n        .y = call(first,\n                  second),\n    };\n}\n";
    check(input, &["--style=kr"], input);
    check(
        input,
        &["--style=allman"],
        "void f()\n{\n    value = (struct Item)\n    {\n        .y = call(first,\n                  second),\n    };\n}\n",
    );
}

#[test]
fn a_qualified_definition_head_takes_its_brace_before_a_trailing_comment() {
    let input = "class Item {\n  public:\n    int value() const /* note */\n    {\n        return value_;\n    }\n    int other() // note\n    {\n        return value_;\n    }\n};\nint g() const /* note */\n{\n    return 0;\n}\n";
    check(
        input,
        &["--style=google"],
        "class Item {\n  public:\n    int value() const { /* note */\n        return value_;\n    }\n    int other() { // note\n        return value_;\n    }\n};\nint g() const { /* note */\n    return 0;\n}\n",
    );
}

#[test]
fn a_lambda_argument_row_with_a_comment_after_its_brace_stands_as_without_it() {
    let input = "void f()\n{\n    std::sort(items.begin(), items.end(),\n    [](const Item &first, const Item &second) { /* note */\n        return first.value < second.value;\n    });\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn indented_brace_styles_set_a_brace_row_a_comment_leads_with_the_brace_rows() {
    // astyle sets the comment at the plain rows' column; the row stands
    // where it would without the comment.
    let input = "static const struct Item items[] = {\n    {1, 2},\n    /* note */ {3,\n     4},\n    {5,\n     6}\n};\n";
    check(
        input,
        &["--style=whitesmith"],
        "static const struct Item items[] = {\n        {1, 2},\n        /* note */ {\n        3,\n        4\n        },\n        {\n        5,\n        6\n        }\n    };\n",
    );
    check(
        input,
        &["--style=ratliff"],
        "static const struct Item items[] = {\n        {1, 2},\n        /* note */ {\n        3,\n        4\n        },\n        {\n        5,\n        6\n        }\n    };\n",
    );
}

#[test]
fn a_shift_row_of_a_statement_a_comment_leads_after_a_stream_stays_in_its_parens() {
    let input = "void f()\n{\n    out << value << ((b + (c - 1))\n                     << c);\n    /* c */ m = 1 << (((p & (size - 1))\n                       >> shift)\n                      + OFFSET);\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_comment_line_between_declarators_stands_at_the_declarator_after_it() {
    let input = "void f()\n{\n    int g = 1,\n        /* note */\n        h = 2;\n    int a = 1,\n#ifdef FEATURE\n        /* note */\n        b = 2,\n#endif\n        c = 3;\n}\nint x = 1,\n    /* note */\n    y = 2;\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_comment_line_before_a_class_base_colon_leaves_the_clause_as_without_it() {
    let input = "struct Item\n    /* note */\n    : public Base,\n      private Other {\n};\n";
    check(input, &["--style=kr"], input);
    check(
        input,
        &["--style=allman"],
        "struct Item\n    /* note */\n    : public Base,\n      private Other\n{\n};\n",
    );
}

#[test]
fn a_comment_line_in_nested_parens_of_a_condition_stands_at_the_condition_rows() {
    let input = "void f(void)\n{\n    if((a ? b :\n            /* note */\n            c) != -1) {\n    }\n    if( (first &&\n            /* note */\n            (second || third))\n      ) {\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_string_row_after_a_comment_line_in_a_case_block_stands_at_the_comment() {
    let input = "void f()\n{\n    switch (x) {\n    case 1: {\n        static const char *names[] = { \"first\", \"second\",\n                                       /* note */\n                                       \"third\", \"fourth\"\n                                     };\n        break;\n    }\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_declarator_split_from_its_aggregate_type_continues_past_a_trailing_comment() {
    let input = "void f()\n{\n    static struct foo /* note */\n        bar[3];\n}\nstatic struct foo /* note */\n    bar[3];\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn pointer_declarator_rows_after_a_trailing_comment_stay_at_the_first_declarator() {
    let input = "struct Item {\n    robj *a, /* note */\n         *b,\n         *c;\n};\nvoid f()\n{\n    robj *a, /* note */\n         *b;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_comment_line_among_the_rows_of_a_run_in_list_stands_at_the_rows() {
    let input = "void f()\n{\n    int b[] = {{1, 2},\n        /* note */\n        3,\n        4, {5},\n        6\n    };\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_list_in_a_case_block_keeps_its_columns_where_its_line_stands() {
    let input = "void f()\n{\n    switch (x) {\n    case 1: {\n        static const char *names[] = { \"first\", \"second\",\n                                       /* note */\n                                       \"third\", \"fourth\"\n                                     };\n        break;\n    }\n    }\n}\n";
    check(
        input,
        &["--style=whitesmith"],
        "void f()\n    {\n    switch (x)\n        {\n        case 1:\n            {\n            static const char *names[] = { \"first\", \"second\",\n                                           /* note */\n                                           \"third\", \"fourth\"\n                                         };\n            break;\n            }\n        }\n    }\n",
    );
}

#[test]
fn a_comment_led_row_after_a_comment_line_in_a_continuation_stays_in_it() {
    let input = "void f()\n{\n    /* note */ return first ||\n                      /* note */\n                      /* c */ second;\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_comment_line_in_a_subscript_continued_in_a_condition_stands_at_its_rows() {
    let input = "void f()\n{\n    if (table[first +\n              /* note */\n              second] && other) {\n        value=0;\n    }\n}\n";
    check(input, &["--style=kr"], input);
}

#[test]
fn a_comment_line_among_message_rows_stands_at_the_rows() {
    let input = "- (int)total\n{\n    return y + [self.other countWith:1\n                           /* note */\n                           and:2];\n}\n";
    check(input, &[], input);
}

#[test]
fn a_message_keyword_spelled_as_a_word_operator_aligns_as_other_keywords() {
    let input = "- (int)total\n{\n    return y + [self.other countWith:1\n and:2];\n    return y + [self.other countWith:1\n or:2];\n}\n";
    check(
        input,
        &[],
        "- (int)total\n{\n    return y + [self.other countWith:1\n                           and:2];\n    return y + [self.other countWith:1\n                           or:2];\n}\n",
    );
}

#[test]
fn message_rows_align_at_the_keyword_whatever_brackets_precede_on_the_line() {
    let input = "- (int)total\n{\n    x = y + [self.other countWith:1\n using:2];\n    x = a[1] + [self.other countWith:1\n using:2];\n    x = [a b] + [self.other countWith:1\n using:2];\n}\n";
    check(
        input,
        &[],
        "- (int)total\n{\n    x = y + [self.other countWith:1\n                        using:2];\n    x = a[1] + [self.other countWith:1\n                           using:2];\n    x = [a b] + [self.other countWith:1\n                            using:2];\n}\n",
    );
}

#[test]
fn padding_operators_leaves_a_message_keyword_spelled_as_a_word_operator_attached() {
    let input = "- (void)update\n{\n    [self.other countWith:value and:other or:last];\n}\n";
    check(input, &["--pad-oper"], input);
}

#[test]
fn a_long_message_splits_at_a_keyword_spelled_as_a_word_operator_as_at_any_keyword() {
    let input = "- (void)update\n{\n    [self.otherObject countWith:valueNumberOne and:valueNumberTwo or:valueNumberThree];\n}\n";
    check(
        input,
        &["--max-code-length=60", "--break-after-logical"],
        "- (void)update\n{\n    [self.otherObject countWith:valueNumberOne and:\n                      valueNumberTwo or:valueNumberThree];\n}\n",
    );
    check(
        input,
        &["--max-code-length=76"],
        "- (void)update\n{\n    [self.otherObject countWith:valueNumberOne and:valueNumberTwo or:\n                      valueNumberThree];\n}\n",
    );
}

#[test]
fn message_rows_in_any_header_condition_align_at_the_keyword() {
    let input = "- (void)update\n{\n    if ([self.other countWith:1\n using:2]) {\n        go();\n    }\n    while ([self.other countWith:1\n using:2]) {\n        go();\n    }\n}\n";
    check(
        input,
        &[],
        "- (void)update\n{\n    if ([self.other countWith:1\n                    using:2]) {\n        go();\n    }\n    while ([self.other countWith:1\n                       using:2]) {\n        go();\n    }\n}\n",
    );
}

#[test]
fn a_designator_value_continues_under_the_value_in_any_brace_list() {
    check(
        "void f(void)\n{\n    struct Config c = {\n        .value = FLAG_A |\n        FLAG_B,\n        .other = FLAG_A | FLAG_B |\n        FLAG_C,\n        .third = helper(1) +\n        2,\n        .fourth =\n        5,\n        FLAG_A |\n        FLAG_B,\n        [2] = FLAG_A |\n        FLAG_B,\n        .last = FLAG_A\n        | FLAG_B,\n    };\n    helper((struct Config) {\n        .value = FLAG_A |\n        FLAG_B,\n        .other = FLAG_A | FLAG_B |\n        FLAG_C,\n        .third = helper(1) +\n        2,\n        .fourth =\n        5,\n        FLAG_A |\n        FLAG_B,\n        [2] = FLAG_A |\n        FLAG_B,\n        .last = FLAG_A\n        | FLAG_B,\n    });\n}\n",
        &[],
        "void f(void)\n{\n    struct Config c = {\n        .value = FLAG_A |\n                 FLAG_B,\n        .other = FLAG_A | FLAG_B |\n                 FLAG_C,\n        .third = helper(1) +\n                 2,\n        .fourth =\n        5,\n        FLAG_A |\n        FLAG_B,\n        [2] = FLAG_A |\n              FLAG_B,\n        .last = FLAG_A\n                | FLAG_B,\n    };\n    helper((struct Config) {\n        .value = FLAG_A |\n                 FLAG_B,\n        .other = FLAG_A | FLAG_B |\n                 FLAG_C,\n        .third = helper(1) +\n                 2,\n        .fourth =\n        5,\n        FLAG_A |\n        FLAG_B,\n        [2] = FLAG_A |\n              FLAG_B,\n        .last = FLAG_A\n                | FLAG_B,\n    });\n}\n",
    );
    check(
        "void f(void)\n{\n    struct Config c = {\n        .inner = { .value = FLAG_A |\n        FLAG_B },\n    };\n}\n",
        &[],
        "void f(void)\n{\n    struct Config c = {\n        .inner = {\n            .value = FLAG_A |\n                     FLAG_B\n        },\n    };\n}\n",
    );
}

#[test]
fn a_designator_string_value_continues_under_the_value() {
    let input = "void f(void)\n{\n    struct Item item = {\n        .name = \"first \"\n        \"second\",\n        [1] = \"first \"\n        \"second\",\n        \"first \"\n        \"second\",\n    };\n}\n";
    check(
        input,
        &[],
        "void f(void)\n{\n    struct Item item = {\n        .name = \"first \"\n                \"second\",\n        [1] = \"first \"\n              \"second\",\n        \"first \"\n        \"second\",\n    };\n}\n",
    );
}
