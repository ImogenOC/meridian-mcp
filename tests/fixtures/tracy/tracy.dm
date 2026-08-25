/proc/meridian_tracy_init()
	var/library = world.system_type == MS_WINDOWS ? "prof.dll" : "./libprof.so"
	var/result = call_ext(library, "init")("block")
	if(result != "0")
		CRASH("byond-tracy init failed: [result]")

/proc/meridian_profile_work(iterations)
	var/total = 0
	for(var/index in 1 to iterations)
		total += index * index
	return total

/world/New()
	. = ..()
	meridian_tracy_init()
	world.log << "MERIDIAN_TRACY_READY"
	spawn()
		while(TRUE)
			meridian_profile_work(2000)
			sleep(1)
