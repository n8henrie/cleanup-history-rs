use super::*;

#[test]
fn test_parse_args() {
    let history_file = parse_args(&mut ["foo", "bar/baz"].iter()).unwrap();
    assert_eq!(history_file.to_str(), Some("bar/baz"));
    let too_many_args = parse_args(&mut ["foo", "bar/baz", "qux"].iter());
    assert!(too_many_args.is_err());
    let not_enough_args = parse_args(&mut ["foo"].iter());
    assert!(not_enough_args.is_err());
}

#[test]
fn reproduces_clean_history() {
    let input = "#123\n\
                 echo foo\n\
                 #456\n\
                 echo bar\n\
                 ";
    let output = input;
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn takes_last_timestamp() {
    let input = "#123\n\
                 #234\n\
                 echo foo\n\
                 #654\n\
                 #456\n\
                 echo bar\n\
                 ";
    let output = "#234\n\
                  echo foo\n\
                  #456\n\
                  echo bar\n\
                  ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn strips_trailing_timestamps() {
    let input = "#123\n\
                 #234\n\
                 echo foo\n\
                 #654\n\
                 #456\n\
                 ";
    let output = "#234\n\
                  echo foo\n\
                  ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn sorts_commands() {
    let input = "#456\n\
                 echo bar\n\
                 #123\n\
                 echo foo
                 ";
    let output = "#123\n\
                  echo foo\n\
                  #456\n\
                  echo bar\n\
                  ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn removes_duplicate_commands() {
    let input = "#123\n\
                 echo foo\n\
                 #456\n\
                 echo foo\n\
                 ";
    let output = "#456\n\
                  echo foo\n\
                  ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn removes_duplicates_leaving_most_recent() {
    let input = "#456\n\
                 echo foo\n\
                 #123\n\
                 echo foo\n\
                 ";
    let output = "#456\n\
                 echo foo\n\
                 ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn skips_short_commands() {
    let input = "#123\n\
                 echo foo\n\
                 #345\n\
                 cd\n\
                 ";
    let output = "#123\n\
                 echo foo\n\
                 ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn skips_cd_and_ls_to_relative_directory() {
    let input = "#123\n\
                 ls foo\n\
                 #345\n\
                 echo bar\n\
                 #456\n\
                 cd ./baz\n\
                 ";
    let output = "#345\n\
                  echo bar\n\
                  ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn strips_extra_spaces_before_processing() {
    let input = "#456\n\
                 echo foo\n\
                 #345\n\
                    echo     foo  \n\
                 #123\n\
                 \t\t\techo\tfoo\t\t
                 ";
    let output = "#456\n\
                  echo foo\n\
                  ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn joints_multiline_commands() {
    let input = "#456\n\
                 echo foo\n\
                 echo bar\n\
                 ";
    let output = "#456\n\
                  echo foo; echo bar\n\
                  ";
    assert_eq!(clean_history(input).unwrap().to_string(), output);
}

#[test]
fn error_if_empty() {
    let input = "#123\n\
                 #456\n\
                 ";
    let result = clean_history(input);
    assert!(result.is_err());
    assert_eq!(result.err().unwrap().to_string(), "no valid commands");
}
