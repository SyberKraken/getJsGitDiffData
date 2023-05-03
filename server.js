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
    case "NOT IMPLEMENTED":

      break;

    default:
      return 23
      break;
  }
}

app.get('/full_backend_generation', (req, res) => {
  //works
    console.log("BACKEND COMMANDO " + req.query.path + "   " + req.query.factor)
    let factor = req.query.factor
    //default to 23
    if (factor.length < 1){
      factor = 23
    }
    let path_command = '"target/release/gitdiffjson.exe" "repo" "' + req.query.path + '"'

    console.log(path_command)
    let _child1 = childprocess.execSync( path_command, [], {shell:false})
    console.log("done with generation")

    let d3_generation_command = '"target/release/gitdiffjson.exe" "d3" "generatedJson.json" "full" "files" "' + text_to_factor_index(factor) + '" "100"'

    let _child2 = childprocess.execSync(d3_generation_command, [], {shell:false})
    console.log("done with d3 gen")
    //let child = childprocess.exec("cd /dir > your_file.txt")
    //target/release/gitdiffjson.exe "d3" "generatedJson.json" "full" "files" "23" "100"
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
