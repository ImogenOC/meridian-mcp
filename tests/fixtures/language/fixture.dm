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

/// Return a fixed fixture value.
#define MERIDIAN_FIXTURE_VALUE 7

/datum/fixture_symbol_parent
	var/value = MERIDIAN_FIXTURE_VALUE

/datum/fixture_symbol_parent/proc/compute(input)
	var/tmp/configured_warning
	value = input
	return value

/datum/fixture_symbol_parent/proc/shadowed(value)
	return value

/datum/fixture_symbol_parent/child

/datum/fixture_symbol_parent/child/compute(input)
	return ..(input)
