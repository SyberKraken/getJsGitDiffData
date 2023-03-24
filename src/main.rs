use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::io::Write;
use regex::Regex;
//sha,  funcs, age, commit message

fn generate_json(repo_path: &str) -> HashMap<String, Vec<(String, Vec<String>,i32, String)>> {
    let mut sha_to_parsed_diffs:HashMap<String, Vec<(String, Vec<String>,i32, String)>> = HashMap::new();
    let mut errors = 0;
    

    let output = Command::new("git")
        .arg("--git-dir=".to_owned() + repo_path + "/.git")
        .arg("--work-tree=".to_owned() + repo_path)
        .arg("log")
        .arg("--pretty=oneline")
        .output()
        .unwrap()
        .stdout;
    let log_output = String::from_utf8_lossy(&output);
    let mut commits = vec![];
    for line in log_output.lines() {
        let mut parts = line.splitn(2, ' ');
        let sha = parts.next().unwrap();
        let message = parts.next().unwrap();
        commits.push((sha.to_owned(), message.to_owned()));
    }

    let sha_list = commits;

    let mut age = 0;
    for sha in sha_list {
        let diff_output = Command::new("git")
            .arg("--git-dir=".to_owned() + repo_path + "/.git")
            .arg("--work-tree=".to_owned() + repo_path)
            .arg("diff")
            .arg(&sha.0)
            .output();

        let diff = match diff_output {
            Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
            Err(err) => {
                errors += 1;
                println!("Error getting diff for sha {}: {:?}", sha.0, err);
                continue;
            }
        };

        age+=1;
        let parsed_diff = get_functions_from_diff(&diff, age,&sha.1);

        sha_to_parsed_diffs.insert(sha.0.to_owned(), parsed_diff);
    }

    if errors > 0 {
        println!("{} errors in linking of commits", errors);
    }

    sha_to_parsed_diffs
}

//age and message is passthrough
fn get_functions_from_diff(diff: &str, age: i32, message: &String) -> Vec<(String, Vec<String>, i32, String)> {
    let regex = Regex::new(r"function\s+[a-zA-Z0-9_]+\(+[a-zA-Z0-9_:, ]*\)|[a-zA-Z0-9]+\s*=\s*\([a-zA-Z0-9: ]*\)\s*=>|[a-zA-Z0-9]+\s*=\s*async\s*\([a-zA-Z0-9: ]*\)\s*=>").unwrap();
    let name_regex = Regex::new(r"diff --git a/(.*) b").unwrap();
    let mut files_objects: Vec<(String, Vec<String>,i32, String)> = vec![];
    let mut curr_filename = String::new();
    let mut curr_file_functions = vec![];
    
    for line in diff.lines() {
        if let Some(name_match) = name_regex.captures(line) {
            if !curr_filename.is_empty() {
                files_objects.push((curr_filename.clone(), curr_file_functions.clone(), age, message.to_string()));
            }
            curr_filename = name_match[1].to_string();
            curr_file_functions = vec![];
        }
        if let Some(func_match) = regex.find(line) {
            let function_name = func_match.as_str().split('(').next().unwrap().trim().to_string().replace("function", "").replace(" ", "").replace("=>", "").replace(" async","").replace("= ","=").replace(": ",":").replace(") ",")").replace('=',"");
            curr_file_functions.push(function_name);
        }
    }
    if !curr_filename.is_empty() {
        files_objects.push((curr_filename.clone(), curr_file_functions.clone(), age, message.to_string()));
    }
    files_objects
}
fn main() {
    let directory_path = "C:\\Users\\simon\\Documents\\My Web Sites\\datavisualisation\\dv";
    let sha_to_parsed_diffs = generate_json(&directory_path);

    let mut result = HashMap::new();
    for (sha, parsed_diffs) in sha_to_parsed_diffs {
        result.insert(sha, parsed_diffs);
    }

    let json = serde_json::to_string_pretty(&result).unwrap();
    let mut file = fs::File::create("generatedJson.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
}