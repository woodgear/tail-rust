#!/bin/bash

rm ./log
touch ./log
for i in {1..1000}
do
    echo $i
    echo $i >> ./log
    sleep 3
done
