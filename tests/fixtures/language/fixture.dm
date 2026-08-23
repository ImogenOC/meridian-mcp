/** Purpose-written Meridian-MCP fixture. */
/datum/meridian_fixture
	/// Values stored by the fixture.
	var/list/items = list()

/** Return the supplied value.
 * Arguments:
 * * value - value to return
 */
/datum/meridian_fixture/proc/do_work(value)
	return value

/datum/meridian_fixture/child
