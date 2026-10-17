create domain string as text
   constraint is_not_empty
        check (value !~ '^$|^\s+$');