w br p on -- tb _dl_open if nsid
comm
	silent
	f 1
	b if $rax && strcmp(((struct link_map *) $rax)->l_name, __progname_full)
	comm
		silent
		eval "add-symbol-file %s -o %#lx", ((struct link_map *) $rax)->l_name, ((struct link_map *) $rax)->l_addr
		c
	end
	set $bp_dl_open = $bpnum
	define hook-run
		dis $bp_dl_open
	end
	c
end
