*** Settings ***
Documentation       Smoke tests for the CLI.

Library             Process


*** Variables ***
${PROJECT_ROOT}     ${CURDIR}${/}..


*** Test Cases ***
Has Help Text
    [Documentation]    Checks the default output.
    ${result}=    Run Local LFS    --help

    Should Be Equal As Integers    ${result.rc}    0
    # Should Be Equal    ${result.stdout}    local-lfs


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
