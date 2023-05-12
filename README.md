Project data-parsing and generation is ran with a rust-script in folder as well as a nodejs-express server.
It visualizes the most bug-prone files in a repository via treemaps.

To start server: node server.js

The "top folder" page shows the contents of the repository folder by folder prioritized by its contents values.

Click on the rectangles or enter a path in the top field to navigate the structure.

The "just files" page shows the top 100(or different amount if manualy generated) files predicted in teh entire repository with its full path.


Generate new data via "http://localhost:5500/generation" page, where you can run the rust-generation scripts that generate pre-named data files that the visualization shows.

Or you can manually generate a variety of files via the rust script:

Compile rust script with: cargo build --release

Rust-functionality:
run: target/release/gitdiff.json
    (on widows you have to add ".exe" to "target/release/gitdiffjson", rust compiles to non-exe file for mac/linux)
    1st arg chooses mode:

        "repo":
            example run command:  target/release/gitdiffjson "repo" "C:\Downloads\gitrepo\"
            args(in order): "directory_path"
            this mode can run on a folder which contains a git-repo.
            It generates a jsonfile called "generatedJson.json" which contains all commits performed in a git repository with minimally required data to perform the rest of operations in the program.

        "multi_analysis":
            example run command:  target/release/gitdiffjson "multi_analysis" "existingFile.json" "new_analysis"
            args(in order): "json_data_path", "new_file_name", ["print_logs"]
            this mode runs analysis of all factors in the file on specified "json_data_path" which needs to be a file generated by the
            "repo" command. It will create a file containing all factors ranked and their relative performance in terms of how many
            Percentage points better they are at detecting bugs in the specified repository. If the "print_logs" arg exists it will
            also print a log-file with all the runs performed and the exact result of all runs for all factors for the specified
            repository.

        "d3"
            example run command: target/release/gitdiffjson "d3" "existingJson.json" "full" "files" "23" "100"
            arg(in order): "existing Json File" "new filename" "files" "factor(number)" "cuttof(number)"
            This mode runs on data generated form the "repo" mode.
            This mode generates the "containers" folder which contains the entire file structure of the generated data from
            "existing JSON File", it also generates a singular file with name "filename_d3.json" with the arg filename which contains
            the top NR of files in the repo.
            The "files" arg chooses to ignore function-treemap generation
            (!!!!!!!!OBS!!!!!this is the only functional parameter in this arg currently as function-treemap generation is not functional.)
            the "factor" arg is a number and chooses what factor is used to generage both the container folder and singular file.
            the "cuttof" arg chooses how many file items to display in the singular file generated.
            The visualization uses the name "full" for its generation, but you can manually enter a file as a get-parameter in the
            search bar if you want to generate multiple different ones and not have to overwrite it every time you switch between
            them since the express server serves all files in the folder

        There are some more "modes" in the code but they are more for testing or running partial parts of the code or debugging.
