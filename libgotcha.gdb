w br p on -- tb _dl_open if nsid == -1
comm
	silent

	f 1
	b if $rax && *((struct link_map *) $rax)->l_name && strcmp(((struct link_map *) $rax)->l_name, __progname_full)
	set $bp_dl_open = $bpnum
	comm
		silent
		eval "add-symbol-file %s -o %#lx", ((struct link_map *) $rax)->l_name, ((struct link_map *) $rax)->l_addr
		c
	end

	cat l
	set $cp_dl_open = $bpnum
	comm
		silent
		dis $cp_dl_open
		en $bp_dl_open
		c
	end
	define hook-run
		dis $bp_dl_open
		en $cp_dl_open
	end
	dis $cp_dl_open

	c
end
