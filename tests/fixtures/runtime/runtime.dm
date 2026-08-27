world
	New()
		..()
		world.log << "MERIDIAN_MCP_READY"

	Topic(T, Addr, Master, Key)
		if(T == "ping")
			return "pong"
		if(T == "meridian_integrity_mutate")
			world.log << "MERIDIAN_INTEGRITY_PHASE_START"
			text2file("changed during runtime", "tracked-runtime-artifact.txt")
			world.log << "MERIDIAN_INTEGRITY_PHASE_COMPLETE"
			return "mutated"
		return ..()
