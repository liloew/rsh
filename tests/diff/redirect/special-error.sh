# M4 requires special-builtin redirection errors to return 2; dash uses 2, bash --posix uses 1; all abort the script.
: <missing
echo must-not-run
