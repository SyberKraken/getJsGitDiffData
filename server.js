const childprocess = require('child_process');
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

app.get('/full_backend_generation', (req, res) => {
  //works
    console.log("BACKEND COMMANDO " + req.query.path + "   " + req.query.factor)
    let post_repo = () => {

      console.log("done with generation")
    }

    let child = childprocess.exec("/target/release/gitdiffjson.exe repo " + req.query.path)
    //let child = childprocess.exec("type nul > your_file.txt")
    child.stdout.pipe(process.stdout)
    child.on('exit', post_repo)

});

const sleep = ms => new Promise(r => setTimeout(r, ms));
// Serve other files in the path
app.get('*', (req, res) => {
  res.sendFile(path.join(__dirname, req.url));
});

// Start the server
const port = 5500;
app.listen(port, () => {
  console.log(`Server listening on  http://localhost:${port}`);
});
