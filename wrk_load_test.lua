local events = { 'started', 'stopped', 'completed', 'garbage' }
local info_hashes = { '2fa90c59c8072c5a4c54c1f1307dacaeb4c82f0f', '3bbc36a0bcae854bd40c4deec639d4afadf65deb', '8a541fa2db56003884b0acf9c059f6652d5f611c', '93ad92182818954031aa1a3aca9b66af105edb50', 'b5e180851bfc411a630dc2d35f66b220e07190f9' }

-- init random
math.randomseed(os.time())

-- the request function that will run at each request
request = function()

    randomString = makeString(20)

    rand_event = events[math.random(#events)]
    rand_hash = info_hashes[math.random(#info_hashes)]
    uploaded = math.random(0, 10000)
    downloaded = math.random(0, 10000)

    -- define the path that will search for q=%v 9%v being a random number between 0 and 1000)
    url_path = "/announce?info_hash="..rand_hash.."&peer_id="..randomString.."&port=6881&uploaded="..uploaded.."&downloaded="..downloaded.."&left=727955456&event="..rand_event.."&numwant=30&no_peer_id=1&compact=1"

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
