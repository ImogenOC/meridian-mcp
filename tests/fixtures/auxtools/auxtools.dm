/proc/auxtools_stack_trace(message)
	CRASH(message)

/proc/auxtools_expr_stub()
	CRASH("auxtools not loaded")

/proc/enable_debugging(mode, port)
	CRASH("auxtools not loaded")

/world/New()
	. = ..()
	var/dll_path = world.GetConfig("env", "AUXTOOLS_DEBUG_DLL")
	var/result = call_ext(dll_path, "auxtools_init")()
	if(result != "SUCCESS")
		CRASH("auxtools init failed: [result]")
	enable_debugging()
	world.log << "MERIDIAN_AUXTOOLS_READY"
	spawn()
		while(TRUE)
			sleep(1)
