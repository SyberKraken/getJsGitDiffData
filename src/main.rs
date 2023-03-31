#![feature(drain_filter)]

use git2::{Oid, Repository, RepositoryOpenFlags};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::{env, fmt, fs};

//sha,  funcs, age, commit message

fn generate_json(repo_path: &str) -> HashMap<String, Vec<(String, Vec<String>, i32, String)>> {
    let sha_to_parsed_diffs: HashMap<String, Vec<(String, Vec<String>, i32, String)>> =
        HashMap::new();
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
            .template("{bar:40} {pos}/{len} [{elapsed_precise}] ({eta})")
            .unwrap(),
    );

    sha_list.par_iter().enumerate().for_each(|(age, sha)| {
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
            repo.revparse_single("HEAD")
                .expect("Failed to get HEAD")
                .peel_to_tree()
                .expect("Failed to peel to tree")
        };
        let diff = repo
            .diff_tree_to_tree(Some(&tree2), Some(&tree1), None)
            .expect("Failed to diff trees");
        let mut diff_text = Vec::new();
        diff.print(git2::DiffFormat::Patch, |_, _, line| {
            diff_text.extend_from_slice(line.content());
            diff_text.push(b'\n');
            true
        })
        .expect("Failed to print diff");
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
fn get_functions_from_diff(
    diff: &str,
    age: i32,
    message: &String,
) -> Vec<(String, Vec<String>, i32, String)> {
    let regex = Regex::new(r"function\s+[a-zA-Z0-9_]+\(+[a-zA-Z0-9_:, ]*\)|[a-zA-Z0-9]+\s*=\s*\([a-zA-Z0-9: ]*\)\s*=>|[a-zA-Z0-9]+\s*=\s*async\s*\([a-zA-Z0-9: ]*\)\s*=>").unwrap();
    let name_regex = Regex::new(r"diff --git a/(.*) b").unwrap();
    let mut files_objects: Vec<(String, Vec<String>, i32, String)> = vec![];
    let mut curr_filename = String::new();
    let mut curr_file_functions = vec![];

    for line in diff.lines() {
        if let Some(name_match) = name_regex.captures(line) {
            if !curr_filename.is_empty() {
                files_objects.push((
                    curr_filename.clone(),
                    curr_file_functions.clone(),
                    age,
                    message.to_string(),
                ));
            }
            curr_filename = name_match[1].to_string();
            curr_file_functions = vec![];
        } else if let Some(func_match) = regex.find(line) {
            let function_name = func_match
                .as_str()
                .split('(')
                .next()
                .unwrap()
                .trim()
                .to_string()
                .replace("function", "")
                .replace(" ", "")
                .replace("=>", "")
                .replace(" async", "")
                .replace("= ", "=")
                .replace(": ", ":")
                .replace(") ", ")")
                .replace('=', "");
            curr_file_functions.push(function_name);
        }
    }
    if !curr_filename.is_empty() {
        files_objects.push((
            curr_filename.clone(),
            curr_file_functions.clone(),
            age,
            message.to_string(),
        ));
    }
    files_objects
}

//Class part easy acess, mbe move this

#[derive(Serialize, Deserialize)]
struct Function {
    name: String,
    freq_counter: f32,
    bug_counter: f32,
    aged_freq_counter: f32,
    aged_bug_freq_counter: f32,
    oldest_newest: (i32, i32),
}
impl Function {
    fn _new(
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
impl Function {
    fn get_field(&self, n: u32) -> f32 {
        match n {
            0 => self.freq_counter,
            1 => self.bug_counter,
            2 => self.aged_freq_counter,
            3 => self.aged_bug_freq_counter,
            _ => -1.0,
        }
    }
}
impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "    {}: freq={}, bug={}, aged_freq={}, aged_bug_freq={}, oldest_newest={:?}",
            self.name,
            self.freq_counter,
            self.bug_counter,
            self.aged_freq_counter,
            self.aged_bug_freq_counter,
            self.oldest_newest
        )
    }
}
#[derive(Serialize, Deserialize)]
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
    fn get_field(&self, n: u32) -> f32 {
        match n {
            0 => self.freq_counter,
            1 => self.bug_counter,
            2 => self.aged_freq_counter,
            3 => self.aged_bug_freq_counter,
            _ => -1.0,
        }
    }
}
impl File {
    fn _new(
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

    fn _add_function(&mut self, function: Function) {
        self.function_list.insert(function.name.clone(), function);
    }
}
impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "File {}: freq={}, bug={}, aged_freq={}, aged_bug_freq={}, oldest_newest={:?}\n",
            self.name,
            self.freq_counter,
            self.bug_counter,
            self.aged_freq_counter,
            self.aged_bug_freq_counter,
            self.oldest_newest
        )?;
        for function in self.function_list.values() {
            write!(f, "{}\n", function)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct FileList {
    files: HashMap<String, File>,
    max_age: usize,
}
impl FileList {
    fn remove_files_with_no_functions(&mut self) {
        let mut files_to_remove = Vec::new();
        for (file_name, file) in &self.files {
            if file.function_list.is_empty() {
                files_to_remove.push(file_name.clone());
            }
        }
        for file_name in files_to_remove {
            self.files.remove(&file_name);
        }
    }
}

impl FileList {
    fn new(max_age: usize) -> FileList {
        FileList {
            files: (HashMap::new()),
            max_age: max_age,
        }
    }
    fn add_file(
        &mut self,
        filename: &str,
        freq_counter: f32,
        bug_counter: f32,
        aged_freq_counter: f32,
        aged_bug_freq_counter: f32,
        oldest_newest: (i32, i32),
    ) {
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

    fn add_function(
        &mut self,
        filename: &str,
        function_name: &str,
        freq_counter: f32,
        bug_counter: f32,
        aged_freq_counter: f32,
        aged_bug_freq_counter: f32,
        oldest_newest: (i32, i32),
    ) {
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
                    name: function_name.to_string(),
                    freq_counter,
                    bug_counter,
                    aged_freq_counter,
                    aged_bug_freq_counter,
                    oldest_newest,
                };
                file.function_list
                    .insert(function_name.to_string(), function);
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
            file.function_list
                .insert(function_name.to_string(), function);
            self.files.insert(filename.to_string(), file);
        }
    }
}
    /* impl Serialize for FileList {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("files", &self.files)?;
            map.end()
        }
    } */
    impl fmt::Display for FileList {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for file in self.files.values() {
                write!(f, "{}", file)?;
            }
            Ok(())
        }
    }

    //Class part to be equivalent to D3 standard

    #[derive(Debug, Deserialize, Serialize, Clone)]
    struct Child {
        name: String,
        group: String,
        value: f64,
        colname: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct Parent {
        name: String,
        children: Vec<Child>,
        value: f64,
        colname: String,
    }

    impl Parent {
        fn sort_children_by_value(&mut self) {
            self.children
                .sort_by(|b, a| a.value.partial_cmp(&b.value).unwrap());
        }
        fn remove_children_with_ending(&mut self, endings:&Vec<&str>){

            let filter = |name:&str|-> bool{
                for end in endings{
                    if name.ends_with(end){
                        return true;
                    }
                }
                return false;
            };

            let mut i = 0;
            while i < self.children.len() {
                
                if filter( &self.children[i].name) {
                    let val = self.children.remove(i);
                    // your code here
                } else {
                    i += 1;
                }
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct Container {
        name: String,
        children: Vec<Parent>,
    }
    impl Container {
        fn sort_parents_by_total_child_value(&mut self) {
            self.children.sort_by(|a, b| {
                let a_total_value: f64 = a.children.iter().map(|child| child.value).sum();
                let b_total_value: f64 = b.children.iter().map(|child| child.value).sum();
                b_total_value.partial_cmp(&a_total_value).unwrap()
            });
        }

        fn new(
            name: String,
            children: Vec<Parent>,     
        ) -> Container {
            return Container { name: (name), children: (children) };
        }
    }


    fn filelist_to_container(filelist: FileList, field: u32) -> Container {
        let mut container = Container {
        name: "Container".to_string(),
        children: vec![],
        };

        // Create a mapping from function names to their corresponding child objects
        let mut function_children_map: HashMap<String, Child> = HashMap::new();
        for (file_name, file) in &filelist.files {
            for (function_name, function) in &file.function_list {
                let child = function_children_map
                    .entry(function_name.to_owned())
                    .or_insert(Child {
                        name: function_name.to_owned(),
                        group: file_name.to_owned(),
                        value: function.get_field(field) as f64,
                        colname: "level3".to_owned(),
                    });
                child.value += function.freq_counter as f64;
            }
        }

        // Create parent objects and add corresponding child objects
        for (file_name, file) in &filelist.files {
            let mut children: Vec<Child> = vec![];
            for (function_name, _function) in &file.function_list {
                let child: Child = function_children_map.get(function_name).unwrap().clone();
                children.push(child);
            }

            let parent = Parent {
                name: file_name.to_owned(),
                children,
                value: file.get_field(field) as f64,
                colname: "level2".to_owned(),
            };
            container.children.push(parent);
        }

        // Update child objects with parent information
        for parent in &container.children {
            for child in &parent.children {
                let parent_name = parent.name.clone();
                let mut child = function_children_map.get_mut(&child.name).unwrap();
                child.group = parent_name;
            }
        }

        container
    }

    fn filelist_to_container_only_files(filelist: &FileList, field: &str) -> Container {
        let mut parent = Parent {
            name: String::from("Files"),
            children: vec![],
            value: 0.0,
            colname: "level2".to_owned(),
        };
        for (_, file) in &filelist.files {
            let child = Child {
                name: file.name.clone(),
                group: String::from("Files"),
                value: match field {
                    "freq_counter" => file.freq_counter as f64,
                    "bug_counter" => file.bug_counter as f64,
                    "aged_freq_counter" => file.aged_freq_counter as f64,
                    "aged_bug_freq_counter" => file.aged_bug_freq_counter as f64,
                    _ => panic!("Invalid field name"),
                },
                colname: "level3".to_owned(),
            };
            parent.value += child.value;
            parent.children.push(child);
        }
        Container {
            name: String::from("Container"),
            children: vec![parent],
        }
    }
    
    fn generate_full_data_from_repo(directort_path:&String){

}

    fn file_data_map_to_file_list(file_data: HashMap<String, Vec<(String, Vec<String>, i32, String)>>, age_limit: usize, recognized_bugfix_indicators:[regex::Regex; 3]) -> FileList{
    let max_age = file_data.len();
    let mut age_cuttof = age_limit;
    if age_cuttof == 0{age_cuttof = max_age}
    let analyze_age_above = max_age - age_cuttof;
    //let age_filtered_file_data = item for item in file_data if item.1.2 > ageCuttof;
    let mut file_list: FileList = FileList::new(max_age-1);
    for (_, files) in file_data {
        if (files.len() > 0) && (files[0].2) < analyze_age_above as i32 {continue;}
        for (filename, functions, age, message) in files {
            let mut bug_counter = 0.0;
            if recognized_bugfix_indicators.iter().any(|regex| regex.is_match(&message)){ bug_counter+=1.0;};
            file_list.add_file(&filename, 1.0, bug_counter, 0.0/*not done yet */, 0.0/*not done yet */, (age,age));
            for func_name in functions{
                file_list.add_function(&filename, &func_name, 1.0, bug_counter, 0.0/*not done yet */, 0.0/*not done yet */, (age,age))
            }
        }
    }
    file_list
}

//Generation Exec with arg being filepath in quotes, "C:\\Users\\simon\\Documents\\My Web Sites\\datavisualisation\\dv"
//Parsing of generated json is 1:path to json, 2: new filename, 3:age cuttof, 0 indicates no cuttof
fn main() {
    let args: Vec<String> = env::args().collect();
    //let age_to_year_amount = 1000000; //TODO: not implemented
    //let supported_file_types = ["js", "ts", "jsx", "tsx"]; //TODO: not implemented
    let filtered_file_types = vec!["json", "md"];
    let recognized_bugfix_indicators = [
        Regex::new(r"(?i)line-[0-9]+").unwrap(), //upsales confirmed
        Regex::new(r"(?i)bug").unwrap(),         //upsales confirmed, might break on other ones
        Regex::new(r"(?i)hotfix").unwrap(),      //upsales confirmed
    ];
    //Modes: repo, classes, d3, text
    let mode:&str = &args[1];
    match  mode {

        //generate mroe compact textfile listing files+functions in order
        "text" =>{"todo";}        
       
        //generate raw data from git repo
        "repo" =>{
            println!(" generate raw data from git repo");
            // args 2+ : 
            let directory_path = &args[2];
            let sha_to_parsed_diffs = generate_json(&directory_path);
        
            let mut result = HashMap::new();
            for (sha, parsed_diffs) in sha_to_parsed_diffs {
                result.insert(sha, parsed_diffs);
            }
        
            let json = serde_json::to_string_pretty(&result).unwrap();
            let mut file = fs::File::create("generatedJson.json").unwrap();
            file.write_all(json.as_bytes()).unwrap();
            }
        ,

        //Convert file/function objects into d3 treemap parsable json, this in entire structure with files and functions
        //args 2 is string representing if we want files,functions or both
        "d3"=>{
            println!("Convert file/function objects into d3 treemap parsable json");
            // args 2+ : 
            let json_path = &args[2];
            let new_filename = &args[3];
            let sub_mode:&str = &args[4];
            let amount_items_to_show:usize = args[5].parse::<usize>().unwrap();

            let file_string = std::fs::read_to_string(json_path).unwrap();
            let file_list : FileList = serde_json::from_str(&file_string).unwrap();
            let mut container : Container ;

            match sub_mode {
                //"functions"=> {let x = "TODO";}
                "files"=>{
                    container = filelist_to_container_only_files(&file_list, "freq_counter");
                    container.sort_parents_by_total_child_value();}
                "full"=> {
                    container = filelist_to_container(file_list, 0); 
                    container.sort_parents_by_total_child_value();}

                _=> {println!("no matching field for for {} ", mode); return}
            }

            // container is the dataformat for a d3 visualization json
            //Dumb copy making to not implement COPY trait
            let mut copy_container : Container = Container { name: "Container".to_string(), children: (vec![]) };

            for mut p in container.children{
                p.remove_children_with_ending(&filtered_file_types);
                p.sort_children_by_value();
                copy_container.children.push(p);
            }

            copy_container.children.truncate(amount_items_to_show);
                        
            
            let json = serde_json::to_string_pretty(&copy_container).unwrap();
            //d3Data.json hardcoded into visualization atm, TODO: insert new_filename
            let mut file = fs::File::create("d3Data.json").unwrap();
            file.write_all(json.as_bytes()).unwrap();     

        }
        //Parse raw data into file/function objects 
        "classes" =>{
            println!("Parse raw data into file/function objects ");
            // args 2+ : 
            let json_path = &args[2];
            let new_filename = &args[3];
            let age_cuttof_string = &args[4].parse::<i128>().unwrap() ;

            let age_cuttof:usize = *age_cuttof_string as usize;
            let file_string = std::fs::read_to_string(json_path).unwrap();
            let file_data: HashMap<String, Vec<(String, Vec<String>, i32, String)>> = serde_json::from_str(&file_string).unwrap();
        
            let file_list = file_data_map_to_file_list(file_data, age_cuttof, recognized_bugfix_indicators);
            

            //println!("{}", file_list);
            let json = serde_json::to_string_pretty(&file_list).unwrap();
            let mut file = fs::File::create(new_filename.to_owned() + ".json").unwrap();
            file.write_all(json.as_bytes()).unwrap();       
        }
        ,
        _=> println!("no matching branch for {} arguments", args[1])
    }
}
