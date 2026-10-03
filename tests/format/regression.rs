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
        "void f(void)\n{\n    uint8_t value = table[\n                        index + 1u\n                         ];\n    use(value);\n}\n",
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
    let expected = "static void check_value(void)\n{\n    CHECK(run_case(\n              request,\n              sizeof(request),\n    (struct call_case) {\n        .first_active = true,\n        .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n";
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
fn compound_literal_call_argument_reindents_from_argument_column() {
    check(
        "static void check_value(void)\n{\n    CHECK(run_case(\n               request,\n               sizeof(request),\n               (struct call_case) {\n                   .first_active = true,\n                   .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n",
        TEST_SAMPLE_OPTIONS,
        "static void check_value(void)\n{\n    CHECK(run_case(\n              request,\n              sizeof(request),\n    (struct call_case) {\n        .first_active = true,\n        .second_busy = true,\n    },\n    &action\n          ) == RESULT_OK);\n}\n",
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
fn array_bound_row_after_a_closed_bracket_aligns_past_the_open_bracket() {
    let input = "void f(void)\n{\n    buf[k] = special[my_random() %\n        ARRAY_SIZE(special)];\n    x = a[b() %\n        c];\n}\n";
    let expected = "void f(void)\n{\n    buf[k] = special[my_random() %\n                     ARRAY_SIZE(special)];\n    x = a[b() %\n              c];\n}\n";
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
fn comment_trailing_a_switch_brace_stands_in_the_case_bodies() {
    let input = "int f(int c)\n{\n  switch( c ){  /* a\n          ** b */\n    case 1:\n      return 1;\n  }\n}\n";
    let expected = "int f(int c)\n{\n    switch( c ) {\n        /* a\n                ** b */\n    case 1:\n        return 1;\n    }\n}\n";
    check(input, &["--style=kr"], expected);
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
