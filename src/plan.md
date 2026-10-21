
cli_api.rs
cli_execution.rs
objects.rs
runtime.rs


main.rs
              v
cli_api       |
              v
main.rs (runtime creation based on existing state)
              v
cli_execution |
              v
uses "runtime" to manipulate objects state, perform actions (like call). State changes are in memory.
              |
main.rs (saving runtime changed state)

cli_api in ideal case should handle all input validation
cli_execution

waffle create-project my_rest_api --global
                                   ^^ project is not locatedd in the folder


waffle set-local-default my_rest_api
waffle environment create  dev
# creates dev (my_rest_api).conf file, youu can enter stuff in there
waffle load_schema
waffle endpoint -f "GET .../virtual-machines/" alias add get_vm
waffle endpoint get_vm send
waffle endpoint get_vm send org=3 test=5
waffle endpoint get_vm shortcut add standard org=3 test=5
waffle endpoint get_vm shortcut standard
waffle endpoint get_vm shortcut add --global standard org=3 test=5
waffle shortcut standard

waffle --interactive
waffle>go endpoint get_vm
waffle:e:get_vm>

waffle:e:set_vm> payload gen template
> payload edit template
..nvim
> send template
> sure? (post is asked)
>
  ^^^ variable (can be done in command context) (interactive recursive allows only command contexts)

  shortcuts for eerything variable = v etc


waffle e get_vm send
waffle variable t save id

waffle set-project my_rest_api
waffle set-env dev
waffle e list_vm s name=my_vm1 -s id results.0.id
waffle endpoint list_vm send --mod-insert name=my_vm1 --out-jq results.0.id
waffle v r id # 3
waffle e power_off s id=[[id]]




^ interactive mode should be supported!
^ recursive interactive (you can get in waffle endpoint get_vm and still be able to escape
^ vim inside would be cool

waffle my_Rest


## TODO ALPHA1:

-> App/envs basic setup may require manual editing but it's managable OK
-> handle very basic auth (just set token thing) OK
-> Handle headers reasonable TODO 
-> Support okish endpoint+template create/edition flow (may require small edits to files for configuration, templates should be seemingles) TODO
notes: JSON only is OK
-> Any missing thing for it to work u know 
-> setup repo

## Todo ALPHA2 (sketch)
- app env can be done through cli fully
- endpoint/tempaltes manipulation can be done through cli fully
- support good endpoint+template flow
- support okish schema validation, configuring schema may require ediitng files
- support global/workspace apps/evns/globals
## TODO alpha3 (sketch)
- support good schema validation
- support opena api schema file
- support .env variables loading
- support okish variables (workspace)
  waffle variables some_var=1234
  waffle endpoint some send --insert foo={{some_var}} 
  waffle endpoint some_get --templates some_template --save some_var
  ^^ dynamic vars survive multiple consoles etc, without it just env variables coulde be used

