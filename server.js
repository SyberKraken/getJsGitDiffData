const childprocess = require('child_process');
const { dir } = require('console');
const express = require('express');
const path = require('path');

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

app.get('/full_backend_generation', (req, res) => {
  //works
    console.log("BACKEND COMMANDO " + req.query.path + "   " + req.query.factor)

    factor = text_to_factor_index(req.query.factor)
    console.log(factor);
    let path_command = '"target/release/gitdiffjson.exe" "repo" "' + req.query.path + '"'

    console.log(path_command)
    let _child1 = childprocess.execSync( path_command, [], {shell:false})
    console.log("done with generation")

    let d3_generation_command = '"target/release/gitdiffjson.exe" "d3" "generatedJson.json" "full" "files" "' + text_to_factor_index(factor) + '" "100"'

    let _child2 = childprocess.execSync(d3_generation_command, [], {shell:false})
    console.log("done with d3 gen")
    //let child = childprocess.exec("cd /dir > your_file.txt")
    //target/release/gitdiffjson.exe "d3" "generatedJson.json" "full" "files" "26" "100"
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
