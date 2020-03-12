local events = { 'started', 'stopped', 'completed', 'garbage' }

-- init random
math.randomseed(os.time())

-- the request function that will run at each request
request = function()

    randomString = makeString(20)

    rand_event = events[math.random(#events)]
    uploaded = math.random(0, 10000)
    downloaded = math.random(0, 10000)

    -- define the path that will search for q=%v 9%v being a random number between 0 and 1000)
    url_path = "/announce?info_hash=3bbc36a0bcae854bd40c4deec639d4afadf65deb&peer_id="..randomString.."&port=6881&uploaded="..uploaded.."&downloaded="..downloaded.."&left=727955456&event="..rand_event.."&numwant=30&no_peer_id=1&compact=1"

    -- if we want to print the path generated
    --print(url_path)-- Return the request object with the current URL path
   
    return wrk.format("GET", url_path)

end

function makeString(l)
        if l < 1 then return nil end -- Check for l < 1
        local s = "" -- Start string
        for i = 1, l do
            n = math.random(65, 90) -- Generate random number from 65 to 90
            s = s .. string.char(n) -- turn it into character and add to string
        end
        return s -- Return string
end
