#![feature(drain_filter)]

use git2::{Oid, Repository, RepositoryOpenFlags};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::collections::hash_map;
use rayon::{prelude::*, vec};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::hash::Hash;
use std::io::Write as _;
use std::fmt::Write as _;
use std::process::Command;
use std::ptr::null;
use std::sync::{Arc, Mutex};
use std::{env, fmt, fs};

//sha,  funcs, age, commit message
fn get_implemented_nr_of_fields_for_analysis() -> i32 {
    //TODO: this needs to be manualy updated
    return 11;        
}

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
    times_func_got_bugfixed_after_end_of_measuring : i32,
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
            times_func_got_bugfixed_after_end_of_measuring : 0,
        }
    }
}
impl Function {
    fn get_field(&self, n: i32) -> f32 {
        match n {
            0 => return self.freq_counter,
            1 => return self.bug_counter,
            2 => return self.aged_freq_counter,
            3 => return self.aged_bug_freq_counter,
            _ => return -1.0,
        }
    }
}
impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "    {}: bugfixes_after={}, freq={}, bug={}, aged_freq={}, aged_bug_freq={}, oldest_newest={:?}\n",
            self.name,
            self.times_func_got_bugfixed_after_end_of_measuring,
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
    times_file_got_bugfixed_after_end_of_measuring : i32,
    functions_bugfixed_after_file_data: HashMap<String, i32>,
    times_functions_got_bugfiexed_after_file_data: i32,
    repo_max_age: i32,
}
fn get_file_field_name( n: i32) -> String {
    let ret = "ERROR no field for: ".to_owned() + &n.to_string();
    match n { 
        0 => return "frequency".to_string(),
        1 => return "fixed bugs".to_string(),
        2 => return "oldest change".to_string(),
        3 => return "newest change".to_string(),
        4 => return "frequency aged by commit ages".to_string(),
        5 => return "fixed bugs aged by commit ages".to_string(),
        6 => return "frequency aged by most recent newest file change".to_string(),
        7 => return "fixed bugs aged by most recent newest file change".to_string(),
        8 => return "frequency aged by most recent oldest file change ".to_string(),
        9 => return "fixed bugs aged by most recent oldest file change ".to_string(),
        10 => return "custom formula".to_string(),
        _ => return "!!!!!!!!ERROR unknown field!!!!!!!!!!!".to_string()
    }
}
impl File {
    fn get_field(&self, n: i32) -> f32 {
        match n {
            0 => self.freq_counter,
            1 => self.bug_counter,
            2 => self.oldest_newest.0 as f32,
            3 => self.oldest_newest.1 as f32,
            4 => self.aged_freq_counter,
            5 => self.aged_bug_freq_counter,
            6 => self.freq_counter  * self.oldest_newest.1 as f32,
            7 => self.bug_counter  * self.oldest_newest.1 as f32,
            8 => self.freq_counter  * self.oldest_newest.0 as f32,
            9 => self.bug_counter  * self.oldest_newest.0 as f32,
            10 => self.oldest_newest.1 as f32 * self.aged_freq_counter * self.aged_bug_freq_counter,
            _ => -1.0,
        }
    }
    
    fn _insert_function_bugfix(&mut self, function_name:String){
        if self.functions_bugfixed_after_file_data.contains_key(&function_name){
            self.functions_bugfixed_after_file_data.insert(function_name.to_owned(), 
                        self.functions_bugfixed_after_file_data.get(&function_name).unwrap() + 1) ;
        }else{
            self.functions_bugfixed_after_file_data.insert(function_name.to_owned(), 1);
        }
    }
    fn get_sorted_function_vec_by_field(&self, field:i32) -> Vec<&Function>{
        let mut fn_list: Vec<&Function> = self.function_list.values().into_iter().collect();
        fn_list.sort_by(|a,b|{ b.get_field(field).total_cmp(&a.get_field(field))});
        return fn_list;
    }
    fn _new(
        name: String,
        freq_counter: f32,
        bug_counter: f32,
        aged_freq_counter: f32,
        aged_bug_freq_counter: f32,
        oldest_newest: (i32, i32),
        repo_max_age: i32,
    ) -> File {
        File {
            name,
            freq_counter,
            bug_counter,
            aged_freq_counter,
            aged_bug_freq_counter,
            oldest_newest,
            function_list: HashMap::new(),
            times_file_got_bugfixed_after_end_of_measuring : 0,
            functions_bugfixed_after_file_data: HashMap::new(),
            times_functions_got_bugfiexed_after_file_data: 0,
            repo_max_age,
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
            "File {}: file_bugfixes_after={},freq={}, bug={}, aged_freq={}, aged_bug_freq={}, oldest_newest={:?}, total_fixes_in_file={}\n",
            self.name,
            self.times_file_got_bugfixed_after_end_of_measuring,
            self.freq_counter,
            self.bug_counter,
            self.aged_freq_counter,
            self.aged_bug_freq_counter,
            self.oldest_newest,
            self.times_functions_got_bugfiexed_after_file_data
        )?;
        //separated this 
       /*  for function in self.function_list.values() {
            write!(f, "{}\n", function)?;
        } */
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct FileList {
    files: HashMap<String, File>,
    max_age: usize,
    //TODO: add some post-end bugfixes to files or smthn here1
    files_bugfixed_after_file_list: HashMap<String, i32>,
    total_bugfixes_after_file_list: i32,
}
impl FileList {
    fn _insert_bugfix(&mut self, filename:&String){
        if self.files_bugfixed_after_file_list.contains_key(filename){
            self.files_bugfixed_after_file_list.insert(filename.to_owned(), self.files_bugfixed_after_file_list.get(filename).unwrap() + 1) ;
        }else{
            self.files_bugfixed_after_file_list.insert(filename.to_owned(), 1);
        }
    }

    fn _remove_files_with_no_functions(&mut self) {
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
    fn new(max_age: usize) -> FileList {
        FileList {
            files: (HashMap::new()),
            max_age: max_age,
            files_bugfixed_after_file_list: (HashMap::new()),
            total_bugfixes_after_file_list: 0,
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
        repo_max_age: i32,
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
                times_file_got_bugfixed_after_end_of_measuring : 0,
                functions_bugfixed_after_file_data: HashMap::new(),
                times_functions_got_bugfiexed_after_file_data: 0,
                repo_max_age,
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
        repo_max_age: i32,
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
                    times_func_got_bugfixed_after_end_of_measuring : 0,
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
                times_file_got_bugfixed_after_end_of_measuring : 0,
                functions_bugfixed_after_file_data: HashMap::new(),
                times_functions_got_bugfiexed_after_file_data: 0,
                repo_max_age,
            };
            let function = Function {
                name: function_name.to_string(),
                freq_counter,
                bug_counter,
                aged_freq_counter,
                aged_bug_freq_counter,
                oldest_newest,
                times_func_got_bugfixed_after_end_of_measuring : 0,
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

    //OBS!!! no longer prints nested functions
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
        value: f32,
        colname: String,
    }
    impl Child{
        fn new(
            name: String,
            group: String,
            value: f32,
            colname: String, 
        ) -> Child {
            return Child {name: (name),
                group: (group),
                value: (value),
                colname: (colname),  };
        }
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    struct Parent {
        name: String,
        children: Vec<Child>,
        value: f32,
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
                    let _ = self.children.remove(i);
                    
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
                let a_total_value: f32 = a.children.iter().map(|child| child.value).sum();
                let b_total_value: f32 = b.children.iter().map(|child| child.value).sum();
                b_total_value.partial_cmp(&a_total_value).unwrap()
            });
        }

        fn _new(
            name: String,
            children: Vec<Parent>,     
        ) -> Container {
            return Container { name: (name), children: (children) };
        }
    }


fn filelist_to_container(filelist: FileList, field: i32) -> Container {
    /*   let mut container = Container {
        name: "Container".to_string(),
        children: vec![],
    }; */
    let child_vec : Vec<Parent> = vec![] ;
    let pb = ProgressBar::new(filelist.files.len().try_into().unwrap());
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40} {pos}/{len} [{elapsed_precise}] ({eta})")
            .unwrap(),
    );

    // Create parent objects and add corresponding child objects
    
    let shared_container = Arc::new(Mutex::new(child_vec));
    filelist.files.par_iter().for_each(|(file_name, file)|{
        pb.inc(1);
        let mut children: Vec<Child> = vec![];

        for (file_name, file) in &filelist.files {
            for (function_name, function) in &file.function_list {

                let child = Child::new(function_name.to_owned(),
                file_name.to_owned(),
                function.get_field(field), 
                "level3".to_owned() );
                children.push( child );
            }
        }
        
        let parent = Parent {
            name: file_name.to_owned(),
            children,
            value: file.get_field(field),
            colname: "level2".to_owned(),
        };
        //container.children.push(parent);
        let mut data = shared_container.lock().unwrap();
        data.push(parent);
        drop(data)
    }) ;
    //this makes us wait for all to finish
    filelist.files.par_iter().for_each(|_| {});
    
    let data = shared_container.lock().unwrap().clone();
    let container = Container {
        name: "Container".to_string(),
        children: data,
    };
    return container;
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
                "freq_counter" => file.freq_counter as f32,
                "bug_counter" => file.bug_counter as f32,
                "aged_freq_counter" => file.aged_freq_counter as f32,
                "aged_bug_freq_counter" => file.aged_bug_freq_counter as f32,
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
    

//This function does all the counting of factors we want to extract form the generated data of commits
fn file_data_map_to_file_list(
                            file_data: &HashMap<String,
                            Vec<(String, Vec<String>, i32, String)>>, 
                            age_limit: usize,
                            recognized_bugfix_indicators:&[regex::Regex; 3]) -> FileList{
    let max_age = file_data.len();

    let age_precentage_to_int: i32 = (max_age as f32 * (age_limit as f32 /100.0)) as i32;
    //println!("cuttof: {},\n{}\n{}\n{}", age_precentage_to_int, max_age, age_limit, age_limit as f32 /100.0);


   /*  let mut age_cuttof = age_limit;
    if age_cuttof == 0{age_cuttof = max_age}
    let analyze_age_above = max_age - age_cuttof;
 */
    //let age_filtered_file_data = item for item in file_data if item.1.2 > ageCuttof;
    let mut file_list: FileList = FileList::new(max_age-1);
    //"files" represents a commit
    for (_, files) in file_data {
        //println!("{}, {}", files[0].2,analyze_age_above as i32 );
        if (files.len() > 0) && (files[0].2) > age_precentage_to_int as i32 {
            // post-cuttof functionality counts bugg fixed after cuttoff
            for (filename, functions, _age, message) in files {
                //TODO: filter other types here as well, list in main mbe.
                if filename.ends_with(".json") || filename.ends_with(".JSON"){continue;};

                if recognized_bugfix_indicators.iter().any(|regex| regex.is_match(&message)){ 
                    //If we are bugfix
                    //if we have a fix on file that didnt exist before cuttof, simply ignore it
                    if !file_list.files.contains_key(filename){ continue;}

                    let changed_file = file_list.files.get_mut(filename).unwrap();
                    file_list.total_bugfixes_after_file_list += 1;
                    changed_file.times_file_got_bugfixed_after_end_of_measuring += 1;

                   // file_list.insert_bugfix(&filename); //old 
                    //increase bug_fixed counter by 1
                    for function in functions{
                        //if newer function than cuttof, ignore
                        if !changed_file.function_list.contains_key(function){continue;}
                        changed_file.times_functions_got_bugfiexed_after_file_data += 1;
                        changed_file.function_list.get_mut(function).unwrap().times_func_got_bugfixed_after_end_of_measuring += 1;
                      
                      /*   let tempname = filename.to_owned();
                        file_list.files.get_mut(&filename).unwrap().
                            insert_function_bugfix(function); */ // old
                    }
                    
                };
              
            }
        }
        else{
            //pre-cuttof functionality adds everything to list from single commit
            for (filename, functions, age, message) in files {
                let mut bug_counter = 0.0;
                if recognized_bugfix_indicators.iter().any(|regex| regex.is_match(&message)){ bug_counter+=1.0;};
                //add_file adds values to existing file if it is in list //TODO: aged currentley adds values form 0 to 1 based on age 
                file_list.add_file(&filename, 1.0, bug_counter, age.to_owned() as f32 /(max_age as f32), bug_counter * (age.to_owned() as f32 /(max_age as f32)), (age.to_owned(),age.to_owned()), file_list.max_age as i32);
                for func_name in functions{
                    //add_function adds values to existing func if it is in list
                    file_list.add_function(&filename, &func_name, 1.0, bug_counter, 0.0/*not done yet */, 0.0/*not done yet */, (age.to_owned(),age.to_owned()), file_list.max_age as i32)
                }
            }
        }
        
    }
    file_list
}

//Generation Exec with arg being filepath in quotes, "C:\\Users\\simon\\Documents\\My Web Sites\\datavisualisation\\dv"
//Parsing of generated json is 1:path to json, 2: new filename, 3:age cuttof, 0 indicates no cuttof
fn main() {
    let mut args: Vec<String> = env::args().collect();
    //let age_to_year_amount = 1000000; //TODO: not implemented
    //let supported_file_types = ["js", "ts", "jsx", "tsx"]; //TODO: not implemented
    let filtered_file_types = vec!["json", "md"];
    let recognized_bugfix_indicators = [
        Regex::new(r"(?i)line-[0-9]+").unwrap(), //upsales confirmed
        Regex::new(r"(?i)bug").unwrap(),         //upsales confirmed, might break on other ones
        Regex::new(r"(?i)hotfix").unwrap(),      //upsales confirmed
        Regex::new(r"(?i)fix:").unwrap(),     //confirmed as standard in vue(v2)
        Regex::new(r"(?i)fix(.*):").unwrap(),     //confirmed as standard in vue(v2)
    ];
    //Modes: repo, classes, d3, text\
    
    if args.len() == 1{ /*assume testing*/ args = vec!["main.rs".to_string(), "d3".to_string(), r"C:\Users\simon\Documents\rust stuff\getJsGitDiffData\upsales.json".to_string(),  "d3Data".to_string(), "full".to_string(), "2".to_string()]; }
    let mode:&str = &args[1];
    match  mode {
        //exclusivley files, runs multi precentage version of text and anylized the data into averages
        "multi_analysis"=>{
            println!("running large multianalysis");
            // args 2+ : 
            let json_data_path = &args[2];
            let file_string = std::fs::read_to_string(json_data_path).unwrap();
            let file_data: HashMap<String, Vec<(String, Vec<String>, i32, String)>> = serde_json::from_str(&file_string).unwrap();
            //This is how much of the repo to include when making a prediction list we make a list of prioritized files for each precentage of the data.
            let precentages = [5,10,15,20,25,30,35,40,45,50,55,60,65,70,75];
             //this is breakpoints for top% of items, so 1 is the top 1% of items sorted by the chosen factor
            let top_list_precentage_breakpoints = [1, 5, 10, 25, 50];
            let nr_of_fields = get_implemented_nr_of_fields_for_analysis();
            //should be , (FREQ, [(10, [1,5,10...]),(15, [1,5,10..])...])
            let mut final_data_labels = vec![];
            let mut final_data :Vec<Vec<(i32,Vec<f64>)>> = Vec::new();



            for index in 0..nr_of_fields{
                final_data_labels.push(get_file_field_name(index));
                final_data.push(vec![]);
                for p in &top_list_precentage_breakpoints{
                    final_data.get_mut(index as usize).unwrap().push((*p, vec![]))
                }
            }
            
            for precentage in precentages{


                let age_cuttof_in_precentage_points = precentage as usize;

                let file_list = file_data_map_to_file_list(&file_data, age_cuttof_in_precentage_points.to_owned(), &recognized_bugfix_indicators);
                
                for i in 0..nr_of_fields{
                    let field_to_sort_by = i;
    
                    let mut sortable_file_vec:Vec<&File> = file_list.files.values().into_iter().collect();
                    
                    //sort files by chosen field
                    sortable_file_vec.sort_by(|a:&&File,b:&&File|{b.get_field(field_to_sort_by).partial_cmp(&a.get_field(field_to_sort_by))}.unwrap());
        
                    let endings_filter = |name:&str|-> bool{
                        for end in &filtered_file_types{
                            if name.ends_with(end){
                                return true;
                            }
                        }
                        return false;
                    };
        
                   
                 
                    let precentages_to_files = top_list_precentage_breakpoints.map(|i|{ return (sortable_file_vec.len() * i as usize)/100});
                    let mut breakpoints_total_bugs_predicted:VecDeque<f32> = VecDeque::with_capacity(top_list_precentage_breakpoints.len());
        
                    let mut precentage_found_count = 0.0;
                    let mut breakpoint_index = 0;
                    let mut index = 0;
                    //currentley only does files
                    for file in sortable_file_vec{
        
                        precentage_found_count += (file.times_file_got_bugfixed_after_end_of_measuring as f32/file_list.total_bugfixes_after_file_list as f32)*100.0;
                        
                        //run check for breakpoints where we list how many % of bugs found
                        if breakpoint_index < precentages_to_files.len() && index == precentages_to_files[breakpoint_index]{
                            breakpoints_total_bugs_predicted.push_back(precentage_found_count);
                            breakpoint_index += 1;
                        }
                        
                        index+=1;
                    }
                    
                    //Push the resuling % of found bugs for each precentage breakpoint(j) for this paticular field(i) 
                    let field_sorted_by_vec_ref= final_data.get_mut(i as usize).unwrap();
                    
                    for j in 0..breakpoints_total_bugs_predicted.len() {
                        field_sorted_by_vec_ref.get_mut(j).unwrap().1.push(breakpoints_total_bugs_predicted[j] as f64)
                        //.push(breakpoints_total_bugs_predicted[j] as f64);
                       // let _ = writeln!(huge_string, "top {}% in list => {}% of bugs predicted", top_list_precentage_breakpoints[j], breakpoints_total_bugs_predicted[j] );
                    }
                    
                }
               
               

            }        

            
            let mut movable_indexes:Vec<usize> = vec![];
            for i in 0..final_data.len(){
                movable_indexes.push(i)
            }

            //Get total deviation from precentages
            let mut movable_index_divergence_total:HashMap<usize, f64> = HashMap::new();

            for (i,label) in final_data_labels.iter().enumerate(){
                movable_index_divergence_total.insert(i.to_owned(), 0.0);
                for title_vector in final_data.get(i){
                    for precentage_pair in title_vector{
                        let avg_sum:f64 = precentage_pair.1.iter().sum();
                        let avg = avg_sum/(precentage_pair.1.len() as f64);
                        *movable_index_divergence_total.get_mut(&i.to_owned()).unwrap() += (avg - precentage_pair.0 as f64)/precentages.len() as f64 ;
                    }
                }
            }

            //sort index_vector for use in sorting other indexed vectors
            movable_indexes.sort_by(|a,b|{
                
                //b.get_field(field_to_sort_by).partial_cmp(&a.get_field(field_to_sort_by))
                let b_div = movable_index_divergence_total.get(b).unwrap();
                let a_div = movable_index_divergence_total.get(a).unwrap();
       
                return b_div.partial_cmp(&a_div).unwrap();
                } 
            );
            //println!("{}", serde_json::to_string_pretty(&movable_indexes).unwrap());
            let mut huge_string:String = String::new();

            for moved_index in movable_indexes{
                writeln!(huge_string,"{} > avg deviation = {}", final_data_labels.get(moved_index).unwrap(), movable_index_divergence_total.get(&moved_index).unwrap());
            }


            //below code is old and should be incorporated with sortablble_indexes, right now we simply print all fo the big data below teh neer metadata
            
            for (i,label) in final_data_labels.iter().enumerate(){
                writeln!(huge_string,"{}", label);
                for title_vector in final_data.get(i){
                    for precentage_pair in title_vector{
                        let avg_sum:f64 = precentage_pair.1.iter().sum();
                        let avg = avg_sum/(precentage_pair.1.len() as f64);
                        writeln!(huge_string,"  {} => {}", &precentage_pair.0, avg);
                    }
                }
             

            }

            let _ = fs::remove_file("macro_analysis.txt");
            let mut file = fs::File::create("macro_analysis.txt").unwrap();
            file.write_all(huge_string.as_bytes()).unwrap();   
        },
        //generate mroe compact textfile from raw data(generated by "repo")
        "text" =>{
            println!("generate compact textfile");
            // args 2+ : 
            let path = &args[2];
            let filename = &args[3];
            let age_cuttof_in_precentage_points:&usize = &args[4].parse::<usize>().unwrap() ;

            let nr_of_fields = get_implemented_nr_of_fields_for_analysis();
            let mut huge_string:String = String::new();

            let file_string = std::fs::read_to_string(path).unwrap();

            let file_data: HashMap<String, Vec<(String, Vec<String>, i32, String)>> = serde_json::from_str(&file_string).unwrap();

            let file_list = file_data_map_to_file_list(&file_data, age_cuttof_in_precentage_points.to_owned(), &recognized_bugfix_indicators);

            //now loops over all into files
            //let field_to_sort_by:i32 = args[5].parse::<i32>().unwrap();
            

            for i in 0..nr_of_fields{
                let field_to_sort_by = i;

                let _ = writeln!(huge_string, "----- {} -----", get_file_field_name(field_to_sort_by));

                let mut sortable_file_vec:Vec<&File> = file_list.files.values().into_iter().collect();
                
                //sort files by chosen field
                sortable_file_vec.sort_by(|a:&&File,b:&&File|{b.get_field(field_to_sort_by).partial_cmp(&a.get_field(field_to_sort_by))}.unwrap());
    
                //append metatdata to top, this should probably be in a var in json later TODO: disabled
               // let _ = write!(huge_string, "total_bugfixes_for_files = {} \n", file_list.total_bugfixes_after_file_list.to_string());
    
                let endings_filter = |name:&str|-> bool{
                    for end in &filtered_file_types{
                        if name.ends_with(end){
                            return true;
                        }
                    }
                    return false;
                };
    
                let top_list_precentage_breakpoints = [1, 5, 10, 25, 50, 75];
                let precentages_to_files = top_list_precentage_breakpoints.map(|i|{ return (sortable_file_vec.len() * i)/100});
                let mut breakpoints_total_bugs_predicted:VecDeque<f32> = VecDeque::with_capacity(top_list_precentage_breakpoints.len());
    
                let mut precentage_found_count = 0.0;
                let mut breakpoint_index = 0;
                let mut index = 0;
                //currentley only does files
                for file in sortable_file_vec{
    
                    precentage_found_count += (file.times_file_got_bugfixed_after_end_of_measuring as f32/file_list.total_bugfixes_after_file_list as f32)*100.0;
                    
                    //run check for breakpoints where we list how many % of bugs found
                    if breakpoint_index < precentages_to_files.len() && index == precentages_to_files[breakpoint_index]{
                        breakpoints_total_bugs_predicted.push_back(precentage_found_count);
                        breakpoint_index += 1;
                    }
                    
    /*                 //filter out json etc. OBS TODO disabled
                    
                
                    let _ = write!(huge_string, "{}", file);
                    //this could be integrated ionto analysis
                    let _ = write!(huge_string, "   file % of total fixes = {} \n", ((file.times_file_got_bugfixed_after_end_of_measuring as f32/file_list.total_bugfixes_after_file_list as f32)*100.0).to_string() );
                    
                    //sort functions by chosen field
                    let func_vec = file.get_sorted_function_vec_by_field(field_to_sort_by);
                                
                    //TODO: currentley only files are analyzes in rpecent ang offther studff
                    for func in func_vec{
                        let _ = write!(huge_string, "{}", func);
    
                    }   */
                    index+=1;
                }
    
                
                for i in 0..breakpoints_total_bugs_predicted.len() {
                    let _ = writeln!(huge_string, "top {}% in list => {}% of bugs predicted", top_list_precentage_breakpoints[i], breakpoints_total_bugs_predicted[i] );
                }
                
    
                let _ = writeln!(huge_string, "\n");
            }
           
           
            //println!("{}",huge_string);
            

            let _ = fs::remove_file(filename.to_owned() + "_fileMap.txt");
            let mut file = fs::File::create(filename.to_owned() + "_fileMap.txt").unwrap();
            
            file.write_all(huge_string.as_bytes()).unwrap();   

        }        
       
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
            //d3Data.json hardcoded into visualization atm
            let _ = fs::remove_file(new_filename.to_owned() + ".json");
            let mut file = fs::File::create(new_filename.to_owned() + ".json").unwrap();
            file.write_all(json.as_bytes()).unwrap();     

        }
        //Parse raw data into file/function objects 
        "classes" =>{
            println!("Parse raw data into file/function objects ");
            // args 2+ : 
            let json_path = &args[2];
            let new_filename = &args[3];
            let age_cuttof_precentage = &args[4].parse::<i128>().unwrap() ;
            let age_cuttof:usize = *age_cuttof_precentage as usize;

            let file_string = std::fs::read_to_string(json_path).unwrap();
            let file_data: HashMap<String, Vec<(String, Vec<String>, i32, String)>> = serde_json::from_str(&file_string).unwrap();
        
            let file_list = file_data_map_to_file_list(&file_data, age_cuttof, &recognized_bugfix_indicators);
            

            //println!("{}", file_list);
            let json = serde_json::to_string_pretty(&file_list).unwrap();
            let mut file = fs::File::create(new_filename.to_owned() + ".json").unwrap();
            file.write_all(json.as_bytes()).unwrap();       
        }
        ,
        _=> println!("no matching branch for {} argument", args[1])
    }
}
