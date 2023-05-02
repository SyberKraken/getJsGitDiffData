const express = require('express');
const path = require('path');

const app = express();

// Serve static files in the current directory
app.use(express.static('.'));

// Serve htmlpage.html for the default path "/"
app.get('/', (req, res) => {
  res.sendFile(path.join(__dirname, 'htmlpage.html'));
});

// Serve other files in the path
app.get('*', (req, res) => {
  res.sendFile(path.join(__dirname, req.url));
});

// Start the server
const port = 5500;
app.listen(port, () => {
  console.log(`Server listening on port ${port}`);
});
