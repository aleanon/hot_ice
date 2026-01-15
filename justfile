default:
    @just --list

run example_name:
    cd examples/{{ example_name }} && just run
