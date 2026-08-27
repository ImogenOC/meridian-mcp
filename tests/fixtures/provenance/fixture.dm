// Source input for the owned provenance fixture.

world
	New()
		..()
		text2file("changed during runtime", "tracked-runtime-artifact.txt")
		world.log << "MERIDIAN_INTEGRITY_PHASE_COMPLETE"
