#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ObservedLineEnding {
    None,
    Lf,
    CrLf,
    Cr,
}

pub(crate) fn normalize(source: &str) -> String {
    source.replace("\r\n", "\n").replace('\r', "\n")
}

pub(crate) fn preferred_line_ending(source: &str) -> ObservedLineEnding {
    let mut crlf = 0usize;
    let mut lf = 0usize;
    let mut cr = 0usize;
    visit_line_endings(source, |ending| match ending {
        ObservedLineEnding::CrLf => crlf += 1,
        ObservedLineEnding::Lf => lf += 1,
        ObservedLineEnding::Cr => cr += 1,
        ObservedLineEnding::None => {}
    });
    if crlf == 0 && lf == 0 && cr == 0 {
        ObservedLineEnding::None
    } else if crlf >= lf {
        if crlf >= cr {
            ObservedLineEnding::CrLf
        } else {
            ObservedLineEnding::Cr
        }
    } else if lf >= cr {
        ObservedLineEnding::Lf
    } else {
        ObservedLineEnding::Cr
    }
}

fn visit_line_endings(source: &str, mut visit: impl FnMut(ObservedLineEnding)) {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ending = match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                index += 2;
                ObservedLineEnding::CrLf
            }
            b'\r' => {
                index += 1;
                ObservedLineEnding::Cr
            }
            b'\n' => {
                index += 1;
                ObservedLineEnding::Lf
            }
            _ => {
                index += 1;
                continue;
            }
        };
        visit(ending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_line_endings() {
        assert_eq!(normalize("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn preferred_line_ending_reports_none_without_line_breaks() {
        assert_eq!(preferred_line_ending("value"), ObservedLineEnding::None);
    }

    #[test]
    fn preferred_line_ending_uses_the_most_common_ending() {
        assert_eq!(
            preferred_line_ending("a\r\nb\r\nc\n"),
            ObservedLineEnding::CrLf
        );
        assert_eq!(preferred_line_ending("a\nb\nc\r\n"), ObservedLineEnding::Lf);
        assert_eq!(preferred_line_ending("a\rb\rc\n"), ObservedLineEnding::Cr);
    }

    #[test]
    fn preferred_line_ending_uses_stable_tie_order() {
        assert_eq!(preferred_line_ending("a\r\nb\n"), ObservedLineEnding::CrLf);
        assert_eq!(preferred_line_ending("a\nb\r"), ObservedLineEnding::Lf);
        assert_eq!(preferred_line_ending("a\r\nb\r"), ObservedLineEnding::CrLf);
    }
}
