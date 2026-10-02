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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n        if(t) {\n            switch(value) {\n            case ONE: {\n                if(ok) {\n                    call(stderr,\n                         \"text\\n\", value);\n                }\n                break;\n            }\n            }\n        }\n}\n",
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
        "void f(void) {\n#ifndef OMIT\n    if( a ) {\n        first();\n    }\n    else\n#endif\n\n        if( n ) {\n            if( arg ) {\n                set();\n            } else {\n                usage();\n                rc = 1;\n            }\n        } else\n\n            if( open ) {\n                int mode = 0;\n                if( safe ) flags = READ;\n\n                for(i=1; i<count; i++) {\n                    const char *item = args[i];\n#ifndef OMIT_OPTION\n                    if( option(item) ) {\n                        mode = 1;\n                    } else\n#endif\n                        if( item[0]=='-' ) {\n                            error();\n                            rc = 1;\n                            goto done;\n                        } else if( name ) {\n                            extra();\n                            rc = 1;\n                            goto done;\n                        } else {\n                            name = item;\n                        }\n                }\n\n                close_all();\n                handle = 0;\n\n                if( name || mode==HEX ) {\n                    if( fresh && name && !safe ) {\n                        if( prefix(name) ) {\n                            char *removed = uri(name);\n                            check(removed);\n                            delete(removed);\n                            free(removed);\n                        } else {\n                            delete(name);\n                        }\n                    }\n#ifndef OMIT_OPTION\n                    if( safe\n                            && mode!=HEX\n                            && name\n                            && compare(name,\":temporary:\")!=0\n                      ) {\n                        fail();\n                    }\n#else\n                    /* comment */\n#endif\n                    if( name ) {\n                        new_name = copy(name);\n                        check(new_name);\n                    } else {\n                        new_name = 0;\n                    }\n                    handle_name = new_name;\n                    open_handle();\n                    if( handle==0 ) {\n                        print();\n                        free(new_name);\n                    } else {\n                        keep = new_name;\n                    }\n                }\n            }\ndone:\n}\n",
    );
}

#[test]
fn preprocessor_else_body_in_split_else_branch_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if( a ){ first(); }else\n#endif\n\n  if( b ){\n#ifndef OMIT_CHECK\n    if( safe\n     && mode\n     && name\n    ){\n      fail();\n    }\n#else\n    /* comment */\n#endif\n    if( name ){\n      open();\n    }else{\n      close();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if( a ) {\n        first();\n    }\n    else\n#endif\n\n        if( b ) {\n#ifndef OMIT_CHECK\n            if( safe\n                    && mode\n                    && name\n              ) {\n                fail();\n            }\n#else\n            /* comment */\n#endif\n            if( name ) {\n                open();\n            } else {\n                close();\n            }\n        }\n}\n",
    );
}

#[test]
fn assignment_logical_continuation_in_deep_split_else_keeps_value_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n  if(b){x();}else\n\n  if(c){x();}else\n\n  if( schema ){\n    int is_schema = same(name, \"one\")==0\n          || same(name, \"two\")==0\n          || same(name, \"three\")==0;\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    }\n    else\n\n        if(b) {\n            x();\n        }\n        else\n\n            if(c) {\n                x();\n            }\n            else\n\n                if( schema ) {\n                    int is_schema = same(name, \"one\")==0\n                                    || same(name, \"two\")==0\n                                    || same(name, \"three\")==0;\n                    call();\n                }\n}\n",
    );
}

#[test]
fn statement_after_assignment_logical_continuation_in_split_else_keeps_block_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n  if( schema ){\n    int is_schema = same(name, \"one\")==0\n                   || same(name, \"two\")==0;\n    if( is_schema ){\n      print(out,\n            \"text\",\n            name\n           );\n    }\n    int glob = find(name, '*') != 0 ||\n               find(name, '?') != 0;\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    }\n    else\n#endif\n\n        if( schema ) {\n            int is_schema = same(name, \"one\")==0\n                            || same(name, \"two\")==0;\n            if( is_schema ) {\n                print(out,\n                      \"text\",\n                      name\n                     );\n            }\n            int glob = find(name, '*') != 0 ||\n                       find(name, '?') != 0;\n        }\n}\n",
    );
}

#[test]
fn conditional_preprocessor_block_after_split_else_keeps_chain_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n#ifndef OMIT\n  if(b){x();}else\n#endif\n\n#ifdef DEBUG\n  /* comment one\n  ** comment two */\n  if(c){\n    x();\n  }else\n#endif\n\n  if(d){\n    y();\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    }\n    else\n\n#ifndef OMIT\n        if(b) {\n            x();\n        }\n        else\n#endif\n\n#ifdef DEBUG\n            /* comment one\n            ** comment two */\n            if(c) {\n                x();\n            } else\n#endif\n\n                if(d) {\n                    y();\n                }\n}\n",
    );
}

#[test]
fn operator_continuation_in_conditional_preprocessor_split_else_keeps_chain_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n#ifndef OMIT\n  if(b){x();}else\n#endif\n\n#ifdef DEBUG\n  /* comment one\n  ** comment two */\n  if(c){\n    x();\n  }else\n#endif\n\n  if(d){\n    if( call(alpha,beta,gamma,delta,epsilon,zeta,eta,theta)\n      != OK ){\n      value = 0;\n    }else{\n      value = 1;\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    }\n    else\n\n#ifndef OMIT\n        if(b) {\n            x();\n        }\n        else\n#endif\n\n#ifdef DEBUG\n            /* comment one\n            ** comment two */\n            if(c) {\n                x();\n            } else\n#endif\n\n                if(d) {\n                    if( call(alpha,beta,gamma,delta,epsilon,zeta,eta,theta)\n                            != OK ) {\n                        value = 0;\n                    } else {\n                        value = 1;\n                    }\n                }\n}\n",
    );
}

#[test]
fn body_after_multiline_else_if_in_conditional_split_else_keeps_header_body_indent() {
    check(
        "void f(void){\n  if(a){x();}else\n\n#ifndef OMIT\n  if(b){x();}else\n#endif\n\n#ifdef DEBUG\n  /* comment one\n  ** comment two */\n  if(c){\n    x();\n  }else\n#endif\n\n  if(d){\n    if( one ){\n      first();\n    }else if( same(z,\"alpha\")==0 || same(z,\"beta\")==0\n           || same(z,\"gamma\")==0 || same(z,\"delta\")==0\n         ){\n      value = get(z);\n    }else if( same(z,\"debug\")==0 ){\n      debug = 1;\n    }else{\n      fail();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n    if(a) {\n        x();\n    }\n    else\n\n#ifndef OMIT\n        if(b) {\n            x();\n        }\n        else\n#endif\n\n#ifdef DEBUG\n            /* comment one\n            ** comment two */\n            if(c) {\n                x();\n            } else\n#endif\n\n                if(d) {\n                    if( one ) {\n                        first();\n                    } else if( same(z,\"alpha\")==0 || same(z,\"beta\")==0\n                               || same(z,\"gamma\")==0 || same(z,\"delta\")==0\n                             ) {\n                        value = get(z);\n                    } else if( same(z,\"debug\")==0 ) {\n                        debug = 1;\n                    } else {\n                        fail();\n                    }\n                }\n}\n",
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
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    }\n    else\n#endif\n\n        if( prompt ) {\n            for(i=0; i<n; i++) {\n                if( option ) {\n                    set();\n                } else\n#ifndef COLOR\n                    if( color ) {\n                        on();\n                    } else\n#endif\n                        if( dash ) {\n                            no();\n                        } else {\n                            bad();\n                        }\n            } else if( extra ) {\n                fail();\n            } else {\n                save();\n            }\n        }\n}\n",
    );
}

#[test]
fn continuation_after_split_else_endif_keeps_branch_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n#ifndef OMIT_BRANCH\n  if( r ){\n    call();\n  }else\n#endif\n\n  if( c=='s' &&\n      (same(arg, \"one\")==0 ||\n       same(arg, \"two\")==0)\n    ){\n    open(\n      arg, value\n    );\n  }else\n\n  if( next ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    }\n    else\n#endif\n\n#ifndef OMIT_BRANCH\n        if( r ) {\n            call();\n        } else\n#endif\n\n            if( c=='s' &&\n                    (same(arg, \"one\")==0 ||\n                     same(arg, \"two\")==0)\n              ) {\n                open(\n                    arg, value\n                );\n            } else\n\n                if( next ) {\n                    next();\n                }\n}\n",
    );
}

#[test]
fn endif_before_else_if_in_split_else_body_keeps_enclosing_if_indent() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n  if( read ){\n    if( pipe ){\n#ifdef OMIT\n      error();\n#else\n      open();\n      if( ok ){\n        read_pipe();\n      }else{\n        fail();\n      }\n#endif\n    }else if( file ){\n      read_file();\n    }else{\n      none();\n    }\n    done();\n  }else\n\n  if( next ){\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    }\n    else\n#endif\n\n        if( read ) {\n            if( pipe ) {\n#ifdef OMIT\n                error();\n#else\n                open();\n                if( ok ) {\n                    read_pipe();\n                } else {\n                    fail();\n                }\n#endif\n            } else if( file ) {\n                read_file();\n            } else {\n                none();\n            }\n            done();\n        } else\n\n            if( next ) {\n                call();\n            }\n}\n",
    );
}

#[test]
fn nested_condition_operator_in_split_else_aligns_to_inner_paren() {
    check(
        "void f(void){\n#ifndef OMIT\n  if(a){x();}else\n#endif\n\n  if( c=='i' && (same(arg, \"one\")==0\n       || same(arg, \"two\")==0)\n  ){\n    call();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT\n    if(a) {\n        x();\n    }\n    else\n#endif\n\n        if( c=='i' && (same(arg, \"one\")==0\n                       || same(arg, \"two\")==0)\n          ) {\n            call();\n        }\n}\n",
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
    }
    else
#endif

        if( a1 ) {
            call1();
        }
        else

            if( a2 ) {
                call2();
            }
            else

#if FEATURE
                if( a3 ) {
                    call3();
                }
                else
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
        "void f(void) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n        if(b1) {\n            x1();\n        }\n        else\n\n            if(t) {\n                static const struct {\n                    const char *name;\n                    int code;\n                } items[] = {\n                    {\"one\",1},\n#ifdef FEATURE\n                    {\"two\",2},\n#endif\n                    {\"three\",3},\n                };\n                done();\n            }\n}\n",
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
        "void f(void) {\n#if FEATURE\n    if( a0 ) {\n        call0();\n    }\n    else\n#endif\n\n        if( a1 ) {\n            call1();\n        }\n        else\n\n            if( a2 ) {\n                call2();\n            }\n            else\n\n#if FEATURE\n                if( a3 ) {\n                    call3();\n                }\n                else\n#endif\n\n                    if( b ) {\n                        if( value[0]=='-'\n                                && (same(value,\"--one\")==0 || same(value,\"-one\")==0)\n                                && n>=4\n                          ) {\n                            x = 1;\n                        }\n\n                        /* comment */\n                        if( same(cmd,\"help\")==0 ) {\n                            call();\n                        }\n                    }\n}\n",
    );
}

#[test]
fn switch_cases_in_deep_split_else_keep_case_body_indent() {
    check(
        "void f(void){\n#if FEATURE\n  if( a0 ){ call0(); }else\n#endif\n\n  if( a1 ){ call1(); }else\n\n  if( a2 ){ call2(); }else\n\n#if FEATURE\n  if( a3 ){ call3(); }else\n#endif\n\n  if( b ){\n    if( bad ){\n      error();\n    }else{\n      switch(value){\n        case ONE: {\n          if( n!=2 ) break;\n          ok = 1;\n          break;\n        }\n        case TWO: {\n          int x;\n          if( n!=3 ) break;\n          x = value();\n          if( x ){\n            use(x);\n          }\n          ok = 2;\n          break;\n        }\n      }\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#if FEATURE\n    if( a0 ) {\n        call0();\n    }\n    else\n#endif\n\n        if( a1 ) {\n            call1();\n        }\n        else\n\n            if( a2 ) {\n                call2();\n            }\n            else\n\n#if FEATURE\n                if( a3 ) {\n                    call3();\n                }\n                else\n#endif\n\n                    if( b ) {\n                        if( bad ) {\n                            error();\n                        } else {\n                            switch(value) {\n                            case ONE: {\n                                if( n!=2 ) break;\n                                ok = 1;\n                                break;\n                            }\n                            case TWO: {\n                                int x;\n                                if( n!=3 ) break;\n                                x = value();\n                                if( x ) {\n                                    use(x);\n                                }\n                                ok = 2;\n                                break;\n                            }\n                            }\n                        }\n                    }\n}\n",
    );
}

#[test]
fn local_struct_in_switch_case_after_split_else_keeps_case_body_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n\n      /* comment */\n      case ONE: {\n        static const struct {\n          int id;\n          const char *name;\n        } items[] = {\n          {1, \"one\"},\n          {2, \"two\"},\n        };\n        done();\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n        if(b1) {\n            x1();\n        }\n        else\n\n            if(t) {\n                switch(value) {\n\n                /* comment */\n                case ONE: {\n                    static const struct {\n                        int id;\n                        const char *name;\n                    } items[] = {\n                        {1, \"one\"},\n                        {2, \"two\"},\n                    };\n                    done();\n                    break;\n                }\n                }\n            }\n}\n",
    );
}

#[test]
fn statement_after_multiline_call_in_case_after_split_else_keeps_block_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n      case ONE: {\n        if( use ){\n          int jj;\n          if( jj>=count ){\n            print(err,\n                  \"Error: %s\\n\", text);\n            puts(\"one\", err);\n            for(jj=0; jj<count; jj++){\n              print(err, \" %s\", labels[jj]);\n            }\n            puts(\"\\n\", err);\n            rc = 1;\n            goto done;\n          }\n        }\n        break;\n      }\n    }\n  }\ndone:\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n        if(b1) {\n            x1();\n        }\n        else\n\n            if(t) {\n                switch(value) {\n                case ONE: {\n                    if( use ) {\n                        int jj;\n                        if( jj>=count ) {\n                            print(err,\n                                  \"Error: %s\\n\", text);\n                            puts(\"one\", err);\n                            for(jj=0; jj<count; jj++) {\n                                print(err, \" %s\", labels[jj]);\n                            }\n                            puts(\"\\n\", err);\n                            rc = 1;\n                            goto done;\n                        }\n                    }\n                    break;\n                }\n                }\n            }\ndone:\n}\n",
    );
}

#[test]
fn commented_struct_members_in_switch_case_after_split_else_keep_case_body_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n      case ONE: {\n        static const struct {\n          unsigned int mask;    /* Mask */\n          unsigned int show;  /* Display */\n          const char *name;   /* Name */\n        } items[] = {\n          { 1, 1, \"one\" },\n        };\n        unsigned int cur;\n        unsigned int next;\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n        if(b1) {\n            x1();\n        }\n        else\n\n            if(t) {\n                switch(value) {\n                case ONE: {\n                    static const struct {\n                        unsigned int mask;    /* Mask */\n                        unsigned int show;  /* Display */\n                        const char *name;   /* Name */\n                    } items[] = {\n                        { 1, 1, \"one\" },\n                    };\n                    unsigned int cur;\n                    unsigned int next;\n                    break;\n                }\n                }\n            }\n}\n",
    );
}

#[test]
fn long_initializer_in_switch_case_after_split_else_keeps_case_body_indent() {
    check(
        "void f(int value){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n  if(b1){ x1(); }else\n\n  if(t){\n    switch(value){\n      case ONE: {\n        static const struct {\n          int id;\n        } items[] = {\n          {1},\n          {2},\n          {3},\n          {4},\n          {5},\n          {6},\n          {7},\n          {8},\n          {9},\n          {10},\n          {11},\n          {12},\n          {13},\n          {14},\n          {15},\n          {16},\n          {17},\n          {18},\n          {19},\n          {20},\n          {21},\n          {22},\n          {23},\n          {24},\n          {25},\n          {26},\n          {27},\n          {28},\n          {29},\n          {30},\n          {31},\n          {32},\n          {33},\n          {34},\n          {35},\n        };\n        for(i=0; i<n; i++){\n          if(i==0){\n            one();\n          }else if(i==1){\n            two();\n          }else{\n            three();\n          }\n        }\n        break;\n      }\n      /* next */\n      case TWO: {\n        if(flag){\n          call(arg,\n               value);\n          done();\n        }\n        break;\n      }\n    }\n  }\n}\n",
        &[],
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n        if(b1) {\n            x1();\n        }\n        else\n\n            if(t) {\n                switch(value) {\n                case ONE: {\n                    static const struct {\n                        int id;\n                    } items[] = {\n                        {1},\n                        {2},\n                        {3},\n                        {4},\n                        {5},\n                        {6},\n                        {7},\n                        {8},\n                        {9},\n                        {10},\n                        {11},\n                        {12},\n                        {13},\n                        {14},\n                        {15},\n                        {16},\n                        {17},\n                        {18},\n                        {19},\n                        {20},\n                        {21},\n                        {22},\n                        {23},\n                        {24},\n                        {25},\n                        {26},\n                        {27},\n                        {28},\n                        {29},\n                        {30},\n                        {31},\n                        {32},\n                        {33},\n                        {34},\n                        {35},\n                    };\n                    for(i=0; i<n; i++) {\n                        if(i==0) {\n                            one();\n                        } else if(i==1) {\n                            two();\n                        } else {\n                            three();\n                        }\n                    }\n                    break;\n                }\n                /* next */\n                case TWO: {\n                    if(flag) {\n                        call(arg,\n                             value);\n                        done();\n                    }\n                    break;\n                }\n                }\n            }\n}\n",
    );
}

#[test]
fn long_preprocessor_split_else_chain_keeps_branch_indent() {
    let depth = 64;
    let mut input = String::from("void f(void){\n#ifndef OMIT\n  if(b0){ x0(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(void) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(void) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(int value) {\n#ifndef OMIT\n    if(b0) {\n        x0();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(b{index}){{ x{index}(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(b{index}) {{\n{indent}    x{index}();\n{indent}}}\n{indent}else\n\n"
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
        "void f(void) {\n#ifndef OMIT_X\n    if( a0 ) {\n        call0();\n    }\n    else\n#endif\n\n        if( a1 ) {\n            call1();\n        }\n        else\n\n            if( a2 ) {\n                call2();\n            }\n            else\n\n#ifndef OMIT_X\n                if( a3 ) {\n                    call3();\n                }\n                else\n#endif\n\n                    if( a4 ) {\n                        if( safe ) {\n                            print(out,\n                                  \"Cannot run command such as \\\"%s\\\" here\\n\",\n                                  arg[0]);\n                            rc = 1;\n                        }\n                    }\n}\n",
    );
}

#[test]
fn close_paren_after_adjacent_string_call_in_split_else_keeps_call_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a0 ){ first(); }else\n#endif\n\n  if( a1 ){ second(); }else\n\n  if( b ){\n    make(out,\n         \"alpha\\n\"\n         \"beta\\n\"\n        );\n    if( flag ){\n      call();\n    }\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a0 ) {\n        first();\n    }\n    else\n#endif\n\n        if( a1 ) {\n            second();\n        }\n        else\n\n            if( b ) {\n                make(out,\n                     \"alpha\\n\"\n                     \"beta\\n\"\n                    );\n                if( flag ) {\n                    call();\n                }\n            }\n}\n",
    );
}

#[test]
fn statement_after_adjacent_string_call_sequence_in_split_else_keeps_block_indent() {
    check(
        "void f(void){\n#ifndef OMIT_X\n  if( a0 ){ first(); }else\n#endif\n\n  if( a1 ){ second(); }else\n\n  if( b ){\n    make(out,\n         \"alpha\\n\"\n         \"beta\\n\"\n        );\n    if( expr ){\n      append(out,\n             \"gamma\\n\", sep);\n      sep = \"AND\";\n    }\n    if( flag ){\n      append(out, \"delta\", sep);\n    }\n    append(out, \"tail\");\n\n    /* comment */\n    if( debug ){\n      print();\n    }else{\n      run();\n    }\n    free(out);\n  }else\n\n  if( c ){\n    next();\n  }\n}\n",
        &[],
        "void f(void) {\n#ifndef OMIT_X\n    if( a0 ) {\n        first();\n    }\n    else\n#endif\n\n        if( a1 ) {\n            second();\n        }\n        else\n\n            if( b ) {\n                make(out,\n                     \"alpha\\n\"\n                     \"beta\\n\"\n                    );\n                if( expr ) {\n                    append(out,\n                           \"gamma\\n\", sep);\n                    sep = \"AND\";\n                }\n                if( flag ) {\n                    append(out, \"delta\", sep);\n                }\n                append(out, \"tail\");\n\n                /* comment */\n                if( debug ) {\n                    print();\n                } else {\n                    run();\n                }\n                free(out);\n            } else\n\n                if( c ) {\n                    next();\n                }\n}\n",
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
        "void f(void) {\n    if( a0 ) {\n        one();\n    }\n    else\n\n        if( a1 ) {\n            one();\n        }\n        else\n\n            if( a2 ) {\n                one();\n            }\n            else\n\n                if( a3 ) {\n                    one();\n                }\n                else\n\n                    if( b ) {\n                        value = call(\n                                    /* note */\n                                    \"alpha\"\n                                    , other);\n                    }\n}\n",
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
        "void f(void)\n{\n    uint8_t value = table[\n                            index + 1u\n                         ];\n    use(value);\n}\n",
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
fn compound_literal_call_argument_uses_statement_indent() {
    let input = "static void check_value(void)\n{\n    CHECK(run_case(\n               request,\n               sizeof(request),\n    (struct call_case) {\n        .first_active = true,\n        .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n";
    check(input, TEST_SAMPLE_OPTIONS, input);
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
fn compound_literal_call_argument_reindents_from_argument_column() {
    check(
        "static void check_value(void)\n{\n    CHECK(run_case(\n               request,\n               sizeof(request),\n               (struct call_case) {\n                   .first_active = true,\n                   .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static void check_value(void)\n{\n    CHECK(run_case(\n               request,\n               sizeof(request),\n    (struct call_case) {\n        .first_active = true,\n        .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n",
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
    check(input, &["--style=kr"], input);
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
        &["--style=1tbs", "--indent-preproc-define", "--pad-header"],
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
