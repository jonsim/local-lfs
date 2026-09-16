*** Settings ***
Documentation       CLI smoke and real Git LFS workflow tests.

Library             Process
Library             LfsWorkflow.py


*** Variables ***
${PROJECT_ROOT}     ${CURDIR}${/}..


*** Test Cases ***
Has Help Text
    [Documentation]    Checks the default output.
    ${result}=    Run Local LFS    --help

    Should Be Equal As Integers    ${result.rc}    0
    Should Contain    ${result.stdout}    usage:

Git LFS Push And Fetch
    [Documentation]    Pushes an LFS object through the pre-push hook, then pulls it into a fresh clone.
    Git Lfs Push And Fetch


*** Keywords ***
Run Local LFS
    [Documentation]    Runs local-lfs with the supplied arguments.
    [Arguments]    @{args}
    ${result}=    Run Process
    ...    cargo    run    --quiet    --
    ...    @{args}
    ...    cwd=${PROJECT_ROOT}
    Should Be Equal As Integers    ${result.rc}    0
    ...    local-lfs failed with return code ${result.rc}\n\n${result.stdout}\n\n${result.stderr}
    RETURN    ${result}
