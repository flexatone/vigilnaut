use crate::util::ResultDynError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

//------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct EnvMarkerExpr {
    pub(crate) left: String,
    pub(crate) operator: String,
    pub(crate) right: String,
}

//------------------------------------------------------------------------------

// os_name 	os.name
// sys_platform 	sys.platform
// platform_machine 	platform.machine() 	x86_64
// platform_python_implementation 	platform.python_implementation()
// platform_release 	platform.release()
// platform_system 	platform.system() 	Linux, Windows, Java
// python_version 	'.'.join(platform.python_version_tuple()[:2])
// python_full_version 	platform.python_version()
// implementation_name 	sys.implementation.name

// NOTE: not implementing "implementation_version", "platform.version", or "extra"
#[derive(Debug)]
struct EnvMarkLeft {
    os_name: String,
    sys_platform: String,
    platform_machine: String,
    platform_python_implementation: String,
    platform_release: String,
    platform_system: String,
    python_version: String,
    python_full_version: String,
    implementation_name: String,
}

const PY_ENV_MARKERS: &str = "import os;import sys;import platform;print(os.name);print(platform.machine());print(platform.python_implementation());print(platform.release());print(platform.system());print('.'.join(platform.python_version_tuple()[:2]));print(platform.python_version());print(sys.implementation.name)";

impl EnvMarkLeft {
    fn from_exe(executable: &Path) -> ResultDynError<Self> {
        let output = Command::new(executable)
            .arg("-S") // disable site on startup
            .arg("-c")
            .arg(PY_ENV_MARKERS)
            .output()?;

        let lines: Vec<_> = std::str::from_utf8(&output.stdout)
            .expect("Failed to convert to UTF-8")
            .trim()
            .lines()
            .collect();
        let os_name = lines[0];
        let sys_platform = lines[0];
        let platform_machine = lines[0];
        let platform_python_implementation = lines[0];
        let platform_release = lines[0];
        let platform_system = lines[0];
        let python_version = lines[0];
        let python_full_version = lines[0];
        let implementation_name = lines[0];

        Ok(EnvMarkLeft {
            os_name,
            sys_platform,
            platform_machine,
            platform_python_implementation,
            platform_release,
            platform_system,
            python_version,
            python_full_version,
            implementation_name,
        })
    }
}

//------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum BExpToken {
    And,
    Or,
    ParenOpen,
    ParenClose,
    Phrase(String), // Arbitrary strings
}

fn bexp_tokenize(expr: &str) -> Vec<BExpToken> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();
    let mut phrase = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '(' => {
                if !phrase.is_empty() {
                    tokens.push(BExpToken::Phrase(phrase.clone()));
                    phrase.clear();
                }
                tokens.push(BExpToken::ParenOpen);
                chars.next();
            }
            ')' => {
                if !phrase.is_empty() {
                    tokens.push(BExpToken::Phrase(phrase.clone()));
                    phrase.clear();
                }
                tokens.push(BExpToken::ParenClose);
                chars.next();
            }
            _ => {
                while let Some(&c) = chars.peek() {
                    if c == ' ' {
                        // only accumulate if not leading
                        if !phrase.is_empty() {
                            phrase.push(c);
                        }
                        chars.next();
                    } else if c != '(' && c != ')' {
                        phrase.push(c);
                        chars.next();

                        if c == 'r' && phrase.ends_with(" or") {
                            let pre_op = phrase[..phrase.len() - 3].trim();
                            if !pre_op.is_empty() {
                                tokens.push(BExpToken::Phrase(pre_op.to_string()));
                            }
                            tokens.push(BExpToken::Or);
                            phrase.clear();
                        } else if c == 'd' && phrase.ends_with(" and") {
                            let pre_op = phrase[..phrase.len() - 4].trim();
                            if !pre_op.is_empty() {
                                tokens.push(BExpToken::Phrase(pre_op.to_string()));
                            }
                            tokens.push(BExpToken::And);
                            phrase.clear();
                        }
                    } else {
                        break; // c is ( or )
                    }
                }
            }
        }
    }
    // Push any remaining phrase
    if !phrase.is_empty() {
        tokens.push(BExpToken::Phrase(phrase.clone()));
    }
    tokens
}

fn bexp_eval(tokens: &[BExpToken], lookup: &HashMap<String, bool>) -> bool {
    let mut index = 0;

    fn eval(
        tokens: &[BExpToken],
        index: &mut usize,
        lookup: &HashMap<String, bool>,
    ) -> bool {
        let mut result = false;
        let mut op = None;

        while *index < tokens.len() {
            match &tokens[*index] {
                BExpToken::Phrase(phrase) => {
                    // println!("phrase: {}", phrase);
                    result = *lookup.get(phrase).unwrap(); // should never happen
                    *index += 1;
                }
                BExpToken::And => {
                    op = Some(BExpToken::And);
                    *index += 1;
                }
                BExpToken::Or => {
                    op = Some(BExpToken::Or);
                    *index += 1;
                }
                BExpToken::ParenOpen => {
                    *index += 1;
                    let sub_result = eval(tokens, index, lookup);
                    if let Some(BExpToken::ParenClose) = tokens.get(*index) {
                        *index += 1;
                    }
                    result = sub_result;
                }
                _ => break,
            }

            if let Some(BExpToken::And) = op {
                result = result && eval(tokens, index, lookup);
            } else if let Some(BExpToken::Or) = op {
                result = result || eval(tokens, index, lookup);
            }
        }
        result
    }
    eval(tokens, &mut index, lookup)
}

//------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bexp_a() {
        let expression = "foo bar or (baz qux and quux corge)";

        let lookup: HashMap<String, bool> = vec![
            ("foo bar".to_string(), true),
            ("baz qux".to_string(), false),
            ("quux corge".to_string(), true),
        ]
        .into_iter()
        .collect();

        let tokens = bexp_tokenize(expression);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, false);
    }

    #[test]
    fn test_bexp_b() {
        let expression = "a or b or c";

        let lookup: HashMap<String, bool> = vec![
            ("a".to_string(), false),
            ("b".to_string(), false),
            ("c".to_string(), true),
        ]
        .into_iter()
        .collect();

        let tokens = bexp_tokenize(expression);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, true);
    }

    #[test]
    fn test_bexp_c() {
        let expression = "a a or b b b b or c c c";

        let lookup: HashMap<String, bool> = vec![
            ("a a".to_string(), false),
            ("b b b b".to_string(), false),
            ("c c c".to_string(), false),
        ]
        .into_iter()
        .collect();

        let tokens = bexp_tokenize(expression);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, false);
    }

    #[test]
    fn test_bexp_d() {
        let expression = "'a a' or ('b b b b' and 'c c c')";

        let lookup: HashMap<String, bool> = vec![
            ("'a a'".to_string(), false),
            ("'b b b b'".to_string(), true),
            ("'c c c'".to_string(), true),
        ]
        .into_iter()
        .collect();

        let tokens = bexp_tokenize(expression);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, true);
    }

    #[test]
    fn test_bexp_e1() {
        let expression = "foo and bar";

        let lookup: HashMap<String, bool> =
            vec![("foo".to_string(), true), ("bar".to_string(), true)]
                .into_iter()
                .collect();

        let tokens = bexp_tokenize(expression);
        println!("{:?}", tokens);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, true);
    }

    #[test]
    fn test_bexp_e2() {
        let expression = "foo and bar";

        let lookup: HashMap<String, bool> =
            vec![("foo".to_string(), true), ("bar".to_string(), false)]
                .into_iter()
                .collect();

        let tokens = bexp_tokenize(expression);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, false);
    }

    #[test]
    fn test_bexp_f1() {
        let expression = "foo and (bar or (baz or (zab or pax)))";

        let lookup: HashMap<String, bool> = vec![
            ("foo".to_string(), true),
            ("bar".to_string(), false),
            ("baz".to_string(), false),
            ("zab".to_string(), false),
            ("pax".to_string(), true),
        ]
        .into_iter()
        .collect();

        let tokens = bexp_tokenize(expression);
        println!("{:?}", tokens);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, true);
    }

    #[test]
    fn test_bexp_f2() {
        let expression = "foo and (bar or (baz or (zab or pax)))";

        let lookup: HashMap<String, bool> = vec![
            ("foo".to_string(), true),
            ("bar".to_string(), false),
            ("baz".to_string(), false),
            ("zab".to_string(), false),
            ("pax".to_string(), false),
        ]
        .into_iter()
        .collect();

        let tokens = bexp_tokenize(expression);
        println!("{:?}", tokens);
        let result = bexp_eval(&tokens, &lookup);
        assert_eq!(result, false);
    }
}
