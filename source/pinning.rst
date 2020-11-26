Pinning
#######

By default, the directory `/studies/climate` will only exist if there are files tagged with both `studies` and `climate`.
This is because tagdirs are tag *intersections.*  If there are no files with both of those tags, that specific directory
path won't exist.  This is useful for navigating directories, because you will only
see sub-tagdirs that are relevant to your current tagdir, but not so useful when populating the tagdirs for the first time.
To alleviate this problem, we use tagdir pinning.

Pinning is creating a temporary "pin" in a tagdir path, so that you can navigate to it without requiring actual files to exist
inside of it.  This allows you to create nested, ad-hoc tagdirs from a "save file" dialog.  Without tagdir pinning, you
could not browse to `/studies/climate` unless there already existed files tagged with `studies` and `climate`.