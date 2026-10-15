trap 'echo caught' USR1; kill -s USR1 $$; echo after
kill -l 15
