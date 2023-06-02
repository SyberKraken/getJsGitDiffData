const del = require('del') ;
const childprocess = require('child_process');
const { dir } = require('console');
const express = require('express');
const path = require('path');
const fs = require('fs');
const app = express();

// Serve static files in the current directory
app.use(express.static('.'));

// Serve htmlpage.html for the default path "/"
app.get('/', (req, res) => {
  res.sendFile(path.join(__dirname, 'htmlpage.html'));
});

app.get('/generation', (req, res) => {
  res.sendFile(path.join(__dirname, 'data_generation.html'));
});

let text_to_factor_index = (text) =>{
  switch (text) {
    case "custom formula bugonly more newest change":return 25
    case "custom formula bugonly":return 18
    case "custom formula bug2 more newest change":return 23
    case "custom formula bug2":return 16
    case "custom formula bug1 more newest change":return 22
    case "custom formula bug1":return 15
    case "custom formula more newest change":return 19
    case "custom formula":return 12
    case "newest change":return 3
    case "custom formula freq1 more newest change":return 20
    case "custom formula freq1":return 13
    case "custom formula freqonly more newest change":return 24
    case "custom formula freqonly":return 17
    case "custom formula freq2 more newest change":return 21
    case "custom formula freq2":return 14
    case "frequency aged by commit ages * newest change":return 10
    case "frequency aged by commit ages":return 4
    case "frequency aged by most recent newest file change":return 6
    case "fixed bugs aged by commit ages * newest change ":return 11
    case "fixed bugs aged by most recent newest file change":return 7
    case "fixed bugs aged by commit ages":return 5
    case "frequency":return 0
    case "fixed bugs":return 1
    case "fixed bugs aged by most recent oldest file change":return 9
    case "frequency aged by most recent oldest file change":return 8
    case "oldest change":return 2
    default:
      return 25

  }
}
//clones adress to temporary repo, returns repo path for later deletion(or not)
let clone_adress = (adressUrl) => {
    // Create a temporary directory to clone the repository int
    const cleanedAddr = adressUrl.replace(/[ &\/\\#,+()$~%.'":*?<>{}]/g, "");
    const repoDir = "./downloaded_repositories/" + cleanedAddr;
    console.log("start cloning")
    // Clone the repository
    childprocess.execSync(`git clone ${adressUrl} ${repoDir}`, [], {shell:false});

    console.log("done with cloning")
    return repoDir
}

let delete_local_directory = (local_dir) =>{
  del.sync([local_dir], { force: true });
}

app.get('/full_backend_generation', (req, res) => {

  let compiled_rust = "target/release/gitdiffjson"
  let platform = process.platform
  if (platform === "win32"){ compiled_rust = compiled_rust + ".exe"}
  //works
  console.log("BACKEND COMMANDO " + req.url)

  let bugfix_regex_list = []

  if(req.query.bugfix_regex){
    //TODO split req.query.bugfix_regex whne working and make into list
    bugfix_regex_list.push(req.query.bugfix_regex) //this is temp to work with oneliners
    console.log("using:" + req.query.bugfix_regex + " bugfix regex")
    //When fixing this remember it needs to be comma separated according to json from frontend
  }
  let bugjson = JSON.stringify(bugfix_regex_list)
  fs.writeFileSync("regex_recognized_bugfixes.json", bugjson);

  let filetype_regex_list = []
  if(req.query.filetype_regex){
    //TODO split req.query.filetype_regex whne working and make into list
    filetype_regex_list.push(req.query.filetype_regex) //this is temp to work with oneliners
    console.log("using:" + req.query.filetype_regex + " filetype regex")
    //When fixing this remember it needs to be comma separated according to json from frontend
  }
  let filejson = JSON.stringify(filetype_regex_list)
  fs.writeFileSync("regex_filtered_file_types.json", filejson);

  let path = req.query.path
  if(req.query.is_remote === "true"){
    path = clone_adress(path)
    console.log("done cloning")
  }


  //run rust parsing on repo path
  console.log("Starting generation")
  factor = text_to_factor_index(req.query.factor)
  let path_command = '"' + compiled_rust + '" "repo" "' + path + '"'
  let _child1 = childprocess.execSync( path_command, [], {shell:false})
  console.log("done with generation")
//fix clone repo fix exe text inline TODO:
  let d3_generation_command = '"' + compiled_rust + '" "d3" "generatedJson.json" "full" "files" "' + text_to_factor_index(factor) + '" "100"'

  let _child2 = childprocess.execSync(d3_generation_command, [], {shell:false})
  console.log("done with d3 gen")


  if(req.query.is_remote === "true"){
    delete_local_directory(path)
    console.log("deleted local repo")
  }
  //let child = childprocess.exec("cd /dir > your_file.txt")
  //target/release/gitdiffjson.exe "d3" "generatedJson.json" "full" "files" "26" "100"
  console.log("finished BACKEND COMMANDO " + req.url)
});

// Serve other files in the path
app.get('*', (req, res) => {
  res.sendFile(path.join(__dirname, req.url));
});

// Start the server
const port = 5500;
app.listen(port, () => {
  console.log(`Server listening on  http://localhost:${port}`);
});
