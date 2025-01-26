-- An example Birocrat script designed to work with JSON-passed questions, where
-- the options in a select question can, depending on what the user picks, lead to
-- other questions. This uses a linked hash table to store the tree structure, and
-- doesn't support multi-selects with dependent questions yet. It also adds two
-- "custom" data types that are interpreted before Birocrat sees them: `number` and
-- `one_ten`.
--
-- I developed this for tracking my habits, where, say, if a question is "Did you
-- meditate today?", and I say yes, I want to follow up with "How long for?"
--
-- See an example JSON structure at `dependent_questions.json`.

-- Returns a deep copy of the given object.
local function deep_copy(orig)
	local orig_type = type(orig)
	local copy

	if orig_type == "table" then
		copy = {}
		for orig_key, orig_value in next, orig, nil do
			copy[deep_copy(orig_key)] = deep_copy(orig_value)
		end
	else
		copy = orig
	end

	return copy
end

-- Recursively adds questions to a 2D linked hash table.
local function add_question(questions, question, next_question, parent_id)
	-- Add the question itself
	questions[question.id] = question
	questions[question.id].next = next_question and next_question.id or nil
	questions[question.id].prnt = parent_id

	-- Check for dependent questions
	if question.type == "select" then
		local indices_to_remove = {}
		for option_name, dep_questions in pairs(question.options) do
			if type(dep_questions) == "table" and #dep_questions > 0 then
				-- Convert the inline table to a child pointer to the first question
				questions[question.id].options[option_name] = dep_questions[1].id

				for idx, dep_q in ipairs(dep_questions) do
					add_question(questions, dep_q, dep_questions[idx + 1], question.id)
				end
			elseif type(dep_questions) == "string" then
				-- We have an option with no dependent questions, set it to be 0
				questions[question.id].options[dep_questions] = 0
				-- We can't remove the array index while iterating
				table.insert(indices_to_remove, option_name)
			else
				-- We have an option with no dependent questions, set it to be 0
				questions[question.id].options[option_name] = 0
			end
		end

		for _, idx in ipairs(indices_to_remove) do
			questions[question.id].options[idx] = nil
		end
	end
end

-- Returns the list of keys in the given table.
local function keys_list(t)
	local keys = {}
	for k, _ in pairs(t) do
		table.insert(keys, k)
	end
	return keys
end

-- Returns the question referenced by `state.active_question` in a format Birocrat understands.
local function question(state)
	local question_obj = deep_copy(state.questions[state.active_question])

	if question_obj.type == "select" then
		-- Transform the options to something workable
		question_obj.options = keys_list(question_obj.options)
		return { "question", question_obj, state }
	elseif question_obj.type == "number" or question_obj.type == "one_ten" then
		-- Take a string and validate later
		question_obj.type = "simple"
		return { "question", question_obj, state }
	else
		-- Birocrat native type
		return { "question", question_obj, state }
	end
end

function Main(state, answer, raw_questions)
	if state == nil then
		local questions = {}
		-- Construct the list from scratch (really important to deep copy, otherwise
		-- our state gets clobbered by persisting table IDs from the immutable params)
		for idx, raw_question in ipairs(deep_copy(raw_questions)) do
			add_question(questions, raw_question, raw_questions[idx + 1], nil)
		end

		state = {
			active_question = raw_questions[1].id,
			questions = questions,
			answers = {},
		}
		return question(state)
	else
		local question_obj = state.questions[state.active_question]
		-- TODO: Support multi-selects with dependent questions
		if question_obj.type == "select" then
			state.answers[state.active_question] = answer.selected[1]

			-- If the answer has dependent questions, go to the first one
			local next_question = question_obj.options[answer.selected[1]]
			if next_question ~= 0 then
				state.active_question = next_question
				return question(state)
			end
		elseif question_obj.type == "number" then
			if tonumber(answer.text) == nil then
				return { "error", "Please enter a number", state }
			end
			state.answers[state.active_question] = answer.text
		elseif question_obj.type == "one_ten" then
			-- Validate the answer
			local answer_num = tonumber(answer.text)
			if answer_num == nil or answer_num < 1 or answer_num > 10 then
				return { "error", "Please enter a number between 1 and 10", state }
			end
			state.answers[state.active_question] = answer.text
		else
			state.answers[state.active_question] = answer.text
		end

		-- If we haven't exited already from an error or dependent question, go to the next
		-- question in this list, or in the parent list, or we're done
		local immediate_next = question_obj.next
		local parent_next = question_obj.prnt and state.questions[question_obj.prnt].next or nil
		if immediate_next ~= nil then
			state.active_question = immediate_next
			return question(state)
		elseif parent_next ~= nil then
			state.active_question = parent_next
			return question(state)
		else
			return { "done", state.answers, state }
		end
	end
end
