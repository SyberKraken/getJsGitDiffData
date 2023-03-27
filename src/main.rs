use std::collections::HashMap;
use std::ffi::OsString;
use std::{fs, env};
use std::sync::{Arc, Mutex};
use std::process::Command;
use std::io::Write;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rayon::prelude::*;
use git2::{Repository, RepositoryOpenFlags, Oid};
//sha,  funcs, age, commit message

fn generate_json(repo_path: &str) -> HashMap<String, Vec<(String, Vec<String>,i32, String)>> {
    let sha_to_parsed_diffs:HashMap<String, Vec<(String, Vec<String>,i32, String)>> = HashMap::new();
    let shared_data = Arc::new(Mutex::new(sha_to_parsed_diffs));
    

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

    let pb = ProgressBar::new(sha_list.len().try_into().unwrap());
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40} {pos}/{len} [{elapsed_precise}] ({eta})").unwrap()
    );
    

    sha_list.par_iter().enumerate().for_each(|(age,sha)| {
        pb.inc(1);

        /*  let diff_output = Command::new("git")
            .arg("--git-dir=".to_owned() + repo_path + "/.git")
            .arg("--work-tree=".to_owned() + repo_path)
            .arg("diff")
            .arg(&sha.0)
            .output();

        let diff = match diff_output {
            Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
            Err(err) => {
                println!("Error getting diff for sha {}: {:?}", sha.0, err);
                return;
            }
        };  */
        // Open the repository
        let repo = match Repository::open_ext(
            repo_path,
            RepositoryOpenFlags::empty(),
            Vec::<OsString>::new(),
        ) {
            Ok(repo) => repo,
            Err(err) => {
                println!("Failed to open repository: {}", err);
                return;
            }
        };

        // Get the commit
        let commit = match repo.find_commit(Oid::from_str(&sha.0).expect("Invalid OID")) {
            Ok(commit) => commit,
            Err(err) => {
                println!("Failed to find commit {}: {}", sha.0, err);
                return;
            }
        };

        // Get the diff
        let tree1 = commit.tree().expect("Failed to get tree");
        let tree2 = if commit.parent_count() > 0 {
            let parent = commit.parent(0).expect("Failed to get parent commit");
            parent.tree().expect("Failed to get parent tree")
        } else {
            repo.revparse_single("HEAD").expect("Failed to get HEAD").peel_to_tree().expect("Failed to peel to tree")
        };
        let diff = repo.diff_tree_to_tree(Some(&tree2), Some(&tree1), None).expect("Failed to diff trees");
        let mut diff_text = Vec::new();
        diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
            diff_text.extend_from_slice(line.content());
            diff_text.push(b'\n');
            true
        }).expect("Failed to print diff");
        let diff_str = String::from_utf8_lossy(&diff_text).to_string();
                                

        let parsed_diff = get_functions_from_diff(&diff_str, age.try_into().unwrap(), &sha.1);


        let mut data = shared_data.lock().unwrap();
        data.insert(sha.0.to_owned(), parsed_diff);
        drop(data)
    });
    //this makes us wait for all to finish
    sha_list.par_iter().for_each(|_| {});

    let data = shared_data.lock().unwrap();
    let returndata = data.clone();
    returndata
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
        else if let Some(func_match) = regex.find(line) {
            let function_name = func_match.as_str().split('(').next().unwrap().trim().to_string().replace("function", "").replace(" ", "").replace("=>", "").replace(" async","").replace("= ","=").replace(": ",":").replace(") ",")").replace('=',"");
            curr_file_functions.push(function_name);
        }
    }
    if !curr_filename.is_empty() {
        files_objects.push((curr_filename.clone(), curr_file_functions.clone(), age, message.to_string()));
    }
    files_objects
}




//Class part, mbe move this
struct Function {
    name: String,
    freq_counter: f32,
    bug_counter: f32,
    aged_freq_counter: f32,
    aged_bug_freq_counter: f32,
    oldest_newest: (i32, i32),
}
impl Function {
    fn new(
        name: String,
        freq_counter: f32,
        bug_counter: f32,
        aged_freq_counter: f32,
        aged_bug_freq_counter: f32,
        oldest_newest: (i32, i32),
    ) -> Function {
        Function {
            name,
            freq_counter,
            bug_counter,
            aged_freq_counter,
            aged_bug_freq_counter,
            oldest_newest,
        }
    }
}

struct File {
    name: String,
    freq_counter: f32,
    bug_counter: f32,
    aged_freq_counter: f32,
    aged_bug_freq_counter: f32,
    oldest_newest: (i32, i32),
    function_list: HashMap<String, Function>,
}
impl File {
    fn new(
        name: String,
        freq_counter: f32,
        bug_counter: f32,
        aged_freq_counter: f32,
        aged_bug_freq_counter: f32,
        oldest_newest: (i32, i32),
    ) -> File {
        File {
            name,
            freq_counter,
            bug_counter,
            aged_freq_counter,
            aged_bug_freq_counter,
            oldest_newest,
            function_list: HashMap::new(),
        }
    }

    fn add_function(&mut self, function: Function) {
        self.function_list.insert(function.name.clone(), function);
    }
}

struct FileList {
    files: HashMap<String, File>,
}

impl FileList {
    fn add_file(&mut self, filename: &str, freq_counter: f32, bug_counter: f32, aged_freq_counter: f32, aged_bug_freq_counter: f32, oldest_newest: (i32, i32)) {
        if let Some(file) = self.files.get_mut(filename) {
            // Update existing file
            file.freq_counter += freq_counter;
            file.bug_counter += bug_counter;
            file.aged_freq_counter += aged_freq_counter;
            file.aged_bug_freq_counter += aged_bug_freq_counter;
            if oldest_newest.0 < file.oldest_newest.0 {
                file.oldest_newest.0 = oldest_newest.0;
            }
            if oldest_newest.1 > file.oldest_newest.1 {
                file.oldest_newest.1 = oldest_newest.1;
            }
        } else {
            // Add new file
            let file = File {
                name: filename.to_string(),
                freq_counter,
                bug_counter,
                aged_freq_counter,
                aged_bug_freq_counter,
                oldest_newest,
                function_list: HashMap::new(),
            };
            self.files.insert(filename.to_string(), file);
        }
    }
    
    fn add_function(&mut self, filename: &str, function_name: &str, freq_counter: f32, bug_counter: f32, aged_freq_counter: f32, aged_bug_freq_counter: f32, oldest_newest: (i32, i32)) {
        if let Some(file) = self.files.get_mut(filename) {
            if let Some(function) = file.function_list.get_mut(function_name) {
                // Update existing function
                function.freq_counter += freq_counter;
                function.bug_counter += bug_counter;
                function.aged_freq_counter += aged_freq_counter;
                function.aged_bug_freq_counter += aged_bug_freq_counter;
                if oldest_newest.0 < function.oldest_newest.0 {
                    function.oldest_newest.0 = oldest_newest.0;
                }
                if oldest_newest.1 > function.oldest_newest.1 {
                    function.oldest_newest.1 = oldest_newest.1;
                }
            } else {
                // Add new function
                let function = Function {
                    name : function_name.to_string(),
                    freq_counter,
                    bug_counter,
                    aged_freq_counter,
                    aged_bug_freq_counter,
                    oldest_newest,
                };
                file.function_list.insert(function_name.to_string(), function);
            }
        } else {
            // Add new file with new function
            let mut file = File {
                name: filename.to_string(),
                freq_counter,
                bug_counter,
                aged_freq_counter,
                aged_bug_freq_counter,
                oldest_newest,
                function_list: HashMap::new(),
            };
            let function = Function {
                name: function_name.to_string(),
                freq_counter,
                bug_counter,
                aged_freq_counter,
                aged_bug_freq_counter,
                oldest_newest,
            };
            file.function_list.insert(function_name.to_string(), function);
            self.files.insert(filename.to_string(), file);
        }
    }
}
//Generation Exec with arg being filepath in quotes, "C:\\Users\\simon\\Documents\\My Web Sites\\datavisualisation\\dv"
//Parsing of generated json is 1:path to json, 2 any input at all.
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 2{
        let json_path = "generatedJson.json";
        //let  json_path = &args[1];

        let file_string = std::fs::read_to_string(json_path).unwrap();
        let file_data: HashMap<String, Vec<(String, Vec<String>, i32, String)>> = serde_json::from_str(&file_string).unwrap();


        for (sha, sha_content) in file_data{

        }

    }else{
        let directory_path = &args[1];
        let sha_to_parsed_diffs = generate_json(&directory_path);
    
        let mut result = HashMap::new();
        for (sha, parsed_diffs) in sha_to_parsed_diffs {
            result.insert(sha, parsed_diffs);
        }
    
        let json = serde_json::to_string_pretty(&result).unwrap();
        let mut file = fs::File::create("generatedJson.json").unwrap();
        file.write_all(json.as_bytes()).unwrap();
    }
 

}