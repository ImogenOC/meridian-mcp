world
	New()
		..()
		world.log << "MERIDIAN_MCP_READY"

	Topic(T, Addr, Master, Key)
		if(T == "ping")
			return "pong"
		return ..()
