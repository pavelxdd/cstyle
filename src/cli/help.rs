use super::CliError;
use std::io::{self, Write};

pub(super) fn print(display: &str) -> Result<(), CliError> {
    writeln!(
        io::stdout().lock(),
        r#"{display}

Usage:
  cstyle [OPTION] [FILE]...
  cstyle < input.c > output.c

Utility and meta options:
  -h, -?, --help                     Print help
  -V, --version                      Print version
  --options=PATH, --options=none     Read formatter-only options from PATH, or disable them
  --project, --project=NAME          Read project formatter options
  --project=none                     Disable project formatter options
  --stdin=PATH                       Read stream input from PATH
  --stdout=PATH                      Write stream output to PATH
  -r, -R, --recursive                Recursively format wildcard targets
  -n, --suffix=none                  Do not create a backup
  --suffix=SUFFIX                    Use SUFFIX for backups
  --dry-run                          Report changes without writing files
  --error-on-changes                 Exit with code 1 on dry-run changes
  --accept-empty-list                Allow empty wildcard matches
  -Q, --formatted                    Display only formatted files
  -q, --quiet                        Suppress file status output
  -v, --verbose                      Display header and summary
  -X, --errors-to-stdout             Print errors to stdout
  -Z, --preserve-date                Preserve file modified time
  --exclude=PATTERN                  Exclude matching file paths
  -i, --ignore-exclude-errors        Show unmatched excludes but continue
  -xi, --ignore-exclude-errors-x     Ignore unmatched excludes

Formatter options:
  --mode=c|objc                      Parse C-family input, or prefer Objective-C ambiguity
  --style=none                       Do not change brace style
  --style=allman|bsd|break|ansi      Allman brace style (-A1)
  --style=java|attach                Attached brace style (-A2)
  --style=kr|k&r|k/r                K&R brace style (-A3)
  --style=linux|knf                  Linux brace style (-A8)
  --style=stroustrup                 Stroustrup brace style (-A4)
  --style=whitesmith                 Whitesmith brace style (-A5)
  --style=ratliff|banner             Ratliff brace style (-A6)
  --style=gnu                        GNU brace style (-A7)
  --style=horstmann|run-in           Horstmann run-in style (-A9)
  --style=1tbs|otbs                  One True Brace Style (-A10)
  --style=pico                       Pico brace style (-A11)
  --style=lisp|python                Lisp/Python brace style (-A12)
  --style=google                     Google brace style (-A14)
  --style=vtk                        VTK brace style (-A15)
  --style=mozilla                    Mozilla brace style (-A16)
  --style=webkit                     WebKit brace style (-A17)
  --indent=spaces=N                  Indent with N spaces (-sN, default N=4)
  --indent=tab=N                     Indent with tabs, N columns each (-tN, default N=4)
  --indent=force-tab=N               Indent with tabs and spaces (-TN, default N=4)
  --indent=force-tab-x=N             Force tabs using tab width N (-xTN, default N=8)
  --indent-continuation=N            Continuation indent 0..4 (-xtN, default N=1)
  --max-continuation-indent=N        Maximum continuation indent 40..120 (-MN)
  --max-instatement-indent=N         Alias for --max-continuation-indent=N
  --min-conditional-indent=N         Conditional indent 0, 1, 2, or 3 (-mN)
  --indent-after-parens              Indent after open parens (-xU)
  --indent-braces                    Indent braces
  --indent-blocks                    Indent whole blocks
  --indent-switches                  Indent switch blocks (-S)
  --indent-cases                     Indent case bodies (-K)
  --indent-labels                    Indent labels (-L)
  --indent-classes                   Indent class blocks (-C)
  --indent-modifiers                 Indent access modifiers (-xG)
  --indent-preprocessor              Indent preprocessor defines (-w)
  --indent-preproc-define            Indent preprocessor defines (-w)
  --indent-preproc-cond              Indent preprocessor conditionals (-xw)
  --indent-preproc-block             Indent preprocessor blocks (-xW)
  --indent-namespaces                Indent namespaces (-N)
  --indent-col1-comments             Indent column-1 comments (-Y)
  --break-one-line-headers           Break one-line headers (-xb)
  --keep-one-line-blocks             Keep one-line blocks (-O)
  --keep-one-line-statements         Keep one-line statements (-o)
  --add-braces, --add-brackets       Add braces to one-line statements (-j)
  --add-one-line-braces              Add one-line braces (-J)
  --add-one-line-brackets            Alias for --add-one-line-braces
  --remove-braces, --remove-brackets Remove braces from one-line statements (-xj)
  --pad-oper                         Pad operators (-p)
  --pad-comma                        Pad commas (-xg)
  --pad-paren                        Pad inside and outside parens (-P)
  --pad-paren-out                    Pad outside parens (-d)
  --pad-first-paren-out              Pad first outside paren only (-xd)
  --pad-paren-in                     Pad inside parens (-D)
  --pad-header                       Pad headers such as if/while (-H)
  --unpad-paren                      Remove extra paren padding (-U)
  --delete-empty-lines               Collapse repeated empty lines (-xe)
  --line-between-members             Add empty lines between members/functions
  --line-between-members=all         Also separate consecutive fields
  --access-label=LABEL               Treat LABEL: as an access/class label
  --macro-block=BEGIN:END            Indent lines between custom macro block delimiters
  --control-header=NAME              Treat NAME(...) as a control statement header
  --non-paren-header=NAME            Treat NAME as a control statement header without parens
  --fill-empty-lines                 Fill empty lines with indentation (-E)
  --remove-comment-prefix            Remove leading comment decoration (-xp)
  --convert-tabs                     Convert tabs to spaces (-c)
  --close-templates                  Close template angle brackets (-xy)
  --align-pointer=type|middle|name   Align pointer operators (-k1, -k2, -k3)
  --align-reference=none|type|middle|name
                                      Align reference operators (-W0, -W1, -W2, -W3)
  --break-after-logical              Break after logical operators (-xL)
  --break-blocks                     Add empty lines around header blocks (-f)
  --break-blocks=all                 Also break closing header blocks (-F)
  --break-closing-braces             Break closing header braces (-y)
  --break-closing-brackets           Alias for --break-closing-braces
  --attach-namespaces                Attach namespace braces (-xn)
  --attach-namespace                 Alias for --attach-namespaces
  --attach-classes                   Attach class braces (-xc)
  --attach-class                     Alias for --attach-classes
  --attach-inlines                   Attach inline braces (-xl)
  --attach-inline                    Alias for --attach-inlines
  --attach-extern-c                  Attach extern "C" braces (-xk)
  --attach-closing-while             Attach do-while closing while (-xV)
  --break-elseifs                    Break else-if chains (-e)
  --no-indent-if-after-else          Do not indent if lines split from else-if
  --break-return-type                Break function return type (-xB)
  --break-return-type-decl           Break declaration return type (-xD)
  --attach-return-type               Attach function return type (-xf)
  --attach-return-type-decl          Attach declaration return type (-xh)
  --max-code-length=N                Split lines longer than N, 50..200 (-xCN)
  --lineend=linux|windows|macold     Force output line endings (-z2, -z1, -z3)
  --pad-method-prefix                Pad Objective-C method prefix (-xQ)
  --unpad-method-prefix              Unpad Objective-C method prefix (-xR)
  --pad-return-type                  Pad Objective-C return type (-xq)
  --unpad-return-type                Unpad Objective-C return type (-xr)
  --pad-param-type                   Pad Objective-C parameter type (-xS)
  --unpad-param-type                 Unpad Objective-C parameter type (-xs)
  --align-method-colon               Align Objective-C method colons (-xM)
  --pad-method-colon=none|all|after|before
                                      Pad Objective-C method colons (-xP0..-xP3)"#,
    )
    .map_err(CliError::stdout)
}
