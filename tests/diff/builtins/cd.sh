CDPATH=search; cd nested; pwd -L; cd -; pwd -P
unset CDPATH; cd -L link; pwd -L; cd ..; cd -P link; pwd -P
