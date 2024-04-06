-- All possible questions we can ask the user
local questions = {
	{
		id = 1,
		type = "simple",
		text = "What is your name, user {id}?",
	},
	{
		id = 2,
		type = "simple",
		text = "How old are you, {name}?",
		default = "30",
	},
	{
		id = 3,
		type = "select",
		text = "What is your favourite type of cuisine?",
		options = { "Indian", "Korean", "Japanese", "Chinese", "Italian" },
	},
	{
		id = 4,
		type = "select",
		text = "What levels of spice can you tolerate?",
		options = { "Mild", "Medium", "Hot", "Very Hot", "Extreme Hot" },
		multiple = true,
	},
}

function Main(state, answer, params)
	-- If this is the first time running the script
	if state == nil and answer == nil then
		-- Return the first question
		local question = questions[1]
		if params.id == nil then
			-- This will fail out the whole form, so there's no need to go back here, hence no need
			-- for a valid state
			return { "error", "No ID parameter provided.", {} }
		end
		question.text = question.text:gsub("{id}", params.id)
		return { "question", question, { question = 1 } }
	end

	if state.question == 1 then
		state.name = answer.text
		state.question = 2
		-- Substitute the user's name in (locally, not globally! pure function!)
		local question = questions[2]
		question.text = question.text:gsub("{name}", state.name)

		return { "question", question, state }
	elseif state.question == 2 then
		state.age = tonumber(answer.text)
		if state.age == nil then
			return { "error", "Please enter a valid number." }
		end
		state.question = 3
		return { "question", questions[3], state }
	elseif state.question == 3 then
		state.favourite_cuisine = answer.selected[1]
		state.question = 4

		if state.favourite_cuisine == "Indian" or state.favourite_cuisine == "Korean" then
			return { "question", questions[4], state }
		else
			return {
				"done",
				{
					name = state.name,
					age = state.age,
					favourite_cuisine = state.favourite_cuisine,
				},
			}
		end
	elseif state.question == 4 then
		state.spice_levels = answer.selected
		return {
			"done",
			{
				name = state.name,
				age = state.age,
				favourite_cuisine = state.favourite_cuisine,
				spice_levels = state.spice_levels,
			},
		}
	end
end
