-- The command station a locomotive is on, by its name. Each device keeps its own record of a station, with its own
-- address on the network, so the name is what the cloud carries. A device with no station of that name leaves it empty
alter table public.locomotives
  add column command_station text;
