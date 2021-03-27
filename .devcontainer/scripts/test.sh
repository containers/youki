#!/bin/bash

test_cases=("default/default.t" "create/create.t" "start/start.t")
# Testing delete and kill is too time consuming.
# test_cases=("default/default.t" "create/create.t" "start/start.t" "delete/delete.t" "kill/kill.t")

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

COLUMNS=$(tput cols) 
expect_err_num=116
act_err_num=0

for case in "${test_cases[@]}"; do
    title="Running $case"
    printf "\n%*s\n" $(((${#title}+$COLUMNS)/2)) "$title"
    IFS=$'\n' errors=($(RUST_BACKTRACE=1 cd /workspaces/runtime-tools && ../runtime-tools/validation/$case | grep "not ok"))

    if [ ${#errors[@]} -eq 0 ]; then
        echo -e "${GREEN}Passed all tess${NC}"
    else
        for err in "${errors[@]}"; do
        act_err_num=$((++act_err_num))
            echo $err
        done
    fi
done

echo 
if [ $act_err_num -ne $expect_err_num ]; then
    echo -e "${RED}The number of failures was as unexpected.${NC}"
    exit 1
fi