package.cpath = package.cpath .. ";./target/debug/lib?.so"

local example = require("dwr")
local client = example.create_client()

local surfaces = {}
local amount = 1
for i = 1, amount do
    client:create_surface(50, 50, function(surface)
        surfaces[i] = surface
    end)
end

local fps = 60
local speed = 100

local top = 0
while client:is_alive() do
    for i = 1, amount do
        local surface = surfaces[i]
        if surface then
            -- surface:set_margin({ top = top + i * 60, bottom = 0, left = 0, right = 0 })
            surface:set_margin({ top = top + i * 60, bottom = 0, right = 0 })
        end
    end
    top = top + speed / fps
    top = top % 800

    client:render()
end
