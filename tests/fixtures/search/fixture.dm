/** Coordinate native canine simulation health and ABI compatibility. */
/datum/controller/subsystem/dogmos_fixture

/** Report native canine library health and ABI compatibility. */
/datum/controller/subsystem/dogmos_fixture/proc/library_status()
	return TRUE

/** Move an atom through the subsystem-managed path queue. */
/datum/move_manager_fixture/proc/queue_path(atom/movable/target)
	return target

/** Store a personal item in a bluespace cache. */
/datum/personal_cache_fixture/proc/store_item(obj/item/stored_item)
	return stored_item

/** Unrelated dog health distractor. */
/mob/living/basic/dog_fixture
	var/health = 100
