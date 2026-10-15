exec 3>f
echo x >&3
exec 3>&-
cat f
pwd >pwdfile
pwd
cat pwdfile
: 0<&- 1>&- 2>&-
echo restored
