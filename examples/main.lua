package.cpath = package.cpath .. ";./target/debug/lib?.so"

local example = require("dwr")
local client = example.create_client()

local surfaces = {}
local amount = 1
for i = 1, amount do
    if client:is_busy() then
        print("can not create surface: ", i)
    end
    surfaces[i] = client:try_create_surface({ width = 50, height = 50 }, function(surface)
        -- print(surface)
        surfaces[i] = surface
        surface:demo_render()
    end)
    -- surfaces[i]:demo_render()
    -- print(surfaces[i])
end

-- while client:is_alive() do
--     client:try_render()
-- end

local fps = 60
local speed = 100

local resized = false

local top = 0
while client:is_alive() do
    for i = 1, amount do
        local surface = surfaces[i]
        if surface then
            surface:set_margin({ top = top + i * 60, bottom = 0, left = 0, right = 0 })
        end
    end
    top = top + speed / fps
    top = top % 800

    for i = 1, amount do
        local surface = surfaces[i]
        if surface then
            if top >= 300 then
                surface:set_size({ width = 50 + top - 300, height = 50 })
                surface:demo_render()
            end
        end
    end

    client:try_render()
end
