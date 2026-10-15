# Non-numeric >& operand: rsh returns 1 per M4; dash aborts as syntax (2); bash --posix redirects both outputs to that filename.
pwd >&not-a-fd
printf '%s\n' "$?"
echo restored
