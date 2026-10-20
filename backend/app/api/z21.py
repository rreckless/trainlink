"""
Z21 Command Station API Endpoints
"""

from fastapi import APIRouter, HTTPException, Depends
from pydantic import BaseModel, Field
from app.drivers.z21_driver import RBUS_GROUPS, get_z21_driver, Z21Driver
from app.drivers.z21_turnouts import MAX_TURNOUT, MIN_TURNOUT, request_turnout_info, throw_turnout
from app.drivers.z21_locos import MAX_FUNCTION, request_loco_info, set_function
from app.services.moving_time import tracker

router = APIRouter(prefix="/api/z21", tags=["z21"])


class Z21Connection(BaseModel):
    """Z21 connection request"""
    host: str = "10.0.0.111"
    port: int = 21105


class LocomotiveCommand(BaseModel):
    """Locomotive control command"""
    address: int
    speed: int
    direction: bool = True


class TurnoutCommand(BaseModel):
    """Turnout output to switch: 0 or 1"""
    output: int = Field(ge=0, le=1)


class FunctionCommand(BaseModel):
    """Switch one function of a locomotive"""
    address: int = Field(ge=1, le=9999)
    function: int = Field(ge=0, le=MAX_FUNCTION)
    on: bool


class EmergencyStopRequest(BaseModel):
    """Emergency stop request"""
    confirm: bool = True


async def get_z21() -> Z21Driver:
    """Dependency to get Z21 driver. Locomotive replies feed the moving time tracker"""
    z21 = await get_z21_driver()
    z21.loco_observer = tracker.observe
    return z21


@router.post("/connect")
async def connect_z21(connection: Z21Connection, z21: Z21Driver = Depends(get_z21)):
    """Connect to Z21 system"""
    z21.host = connection.host
    z21.port = connection.port
    
    success = await z21.connect()
    
    if not success:
        raise HTTPException(
            status_code=500,
            detail=f"Failed to connect to Z21 at {connection.host}:{connection.port}"
        )
    
    return {
        "status": "connected",
        "host": z21.host,
        "port": z21.port,
    }


@router.post("/disconnect")
async def disconnect_z21(z21: Z21Driver = Depends(get_z21)):
    """Disconnect from Z21 system"""
    await z21.disconnect()
    return {"status": "disconnected"}


@router.get("/status")
async def get_z21_status(z21: Z21Driver = Depends(get_z21)):
    """Get Z21 connection and track state.

    Polls the Z21 for its status. The reply updates the state on a later call.
    """
    await z21.get_status()
    return z21.get_connection_status()


@router.get("/feedback")
async def get_z21_feedback(z21: Z21Driver = Depends(get_z21)):
    """Get the last known R-BUS feedback inputs.

    Polls both feedback groups first. Replies update the list on a later call.
    """
    for group in RBUS_GROUPS:
        await z21.request_feedback(group)
    return {
        "connected": z21.connected,
        "inputs": [
            {"module": module, "input": number, "occupied": occupied}
            for (module, number), occupied in sorted(z21.feedback.items())
        ],
    }


@router.post("/turnouts/{turnout}/throw")
async def throw_turnout_output(turnout: int, command: TurnoutCommand, z21: Z21Driver = Depends(get_z21)):
    """Switch a turnout output. The Z21 queues the command and sends it to the track"""
    if not z21.connected:
        raise HTTPException(status_code=503, detail="Z21 not connected")

    if z21.track_state != "ready":
        raise HTTPException(
            status_code=409,
            detail=f"Track is not ready ({z21.track_state}). Turnout commands are blocked",
        )

    if not (MIN_TURNOUT <= turnout <= MAX_TURNOUT):
        raise HTTPException(status_code=400, detail=f"Turnout must be {MIN_TURNOUT}-{MAX_TURNOUT}")

    await throw_turnout(z21, turnout, command.output)
    return {"status": "throw_sent", "turnout": turnout, "output": command.output}


@router.get("/turnouts")
async def get_turnout_states(addresses: str = "", z21: Z21Driver = Depends(get_z21)):
    """Last known output of each turnout listed, as a comma-separated list. Unknown is null.

    Asks the Z21 for each one first. The replies update the states on a later call.
    """
    try:
        turnouts = [int(part) for part in addresses.split(",") if part.strip()]
    except ValueError as error:
        raise HTTPException(status_code=400, detail="Turnouts must be numbers") from error
    if any(not (MIN_TURNOUT <= turnout <= MAX_TURNOUT) for turnout in turnouts):
        raise HTTPException(status_code=400, detail=f"Turnouts must be {MIN_TURNOUT}-{MAX_TURNOUT}")

    if z21.connected:
        for turnout in turnouts:
            await request_turnout_info(z21, turnout)
    return {
        "connected": z21.connected,
        "states": {str(turnout): z21.turnouts.get(turnout) for turnout in turnouts},
    }


@router.post("/loco/function")
async def set_locomotive_function(command: FunctionCommand, z21: Z21Driver = Depends(get_z21)):
    """Switch a locomotive function (F0 is the light)"""
    if not z21.connected:
        raise HTTPException(status_code=503, detail="Z21 not connected")
    if z21.track_state != "ready":
        raise HTTPException(
            status_code=409,
            detail=f"Track is not ready ({z21.track_state}). Locomotive commands are blocked",
        )
    await set_function(z21, command.address, command.function, command.on)
    return {"status": "function_set", "address": command.address, "function": command.function, "on": command.on}


@router.get("/locos")
async def get_locomotive_states(addresses: str = "", z21: Z21Driver = Depends(get_z21)):
    """Last reported state of each locomotive listed, as a comma-separated list. Unknown is null.

    Asks the Z21 for each one first, which also subscribes to its replies. The replies update the states on a later call.
    """
    try:
        locos = [int(part) for part in addresses.split(",") if part.strip()]
    except ValueError as error:
        raise HTTPException(status_code=400, detail="Addresses must be numbers") from error
    if any(not (1 <= address <= 9999) for address in locos):
        raise HTTPException(status_code=400, detail="Addresses must be 1-9999")

    if z21.connected:
        for address in locos:
            await request_loco_info(z21, address)
    tracker.flush_if_due()
    return {
        "connected": z21.connected,
        "states": {str(address): z21.locos.get(address) for address in locos},
    }


@router.post("/track-power/on")
async def track_power_on(z21: Z21Driver = Depends(get_z21)):
    """Turn track power on after a short circuit or manual power off.

    Refused while the Z21 still reports a short circuit.
    """
    if not z21.connected:
        raise HTTPException(status_code=503, detail="Z21 not connected")

    if z21.track_state == "short_circuit":
        raise HTTPException(
            status_code=409,
            detail="Short circuit still reported. Fix the short, then resume track power",
        )

    await z21.track_power_on()
    return {"status": "track_power_on_sent"}


@router.post("/loco/speed")
async def set_locomotive_speed(
    command: LocomotiveCommand,
    z21: Z21Driver = Depends(get_z21)
):
    """Set locomotive speed and direction"""
    if not z21.connected:
        raise HTTPException(status_code=503, detail="Z21 not connected")

    if z21.track_state != "ready":
        raise HTTPException(
            status_code=409,
            detail=f"Track is not ready ({z21.track_state}). Locomotive commands are blocked",
        )

    if not (0 <= command.speed <= 126):
        raise HTTPException(status_code=400, detail="Speed must be 0-126")
    
    if not (1 <= command.address <= 9999):
        raise HTTPException(status_code=400, detail="Address must be 1-9999")
    
    success = await z21.set_loco_speed(
        command.address,
        command.speed,
        command.direction
    )
    
    if not success:
        raise HTTPException(status_code=500, detail="Failed to set locomotive speed")
    tracker.observe(command.address, command.speed)
    
    return {
        "status": "speed_set",
        "address": command.address,
        "speed": command.speed,
        "direction": "forward" if command.direction else "reverse",
    }


@router.post("/loco/info")
async def get_locomotive_info(address: int, z21: Z21Driver = Depends(get_z21)):
    """Get locomotive information from Z21"""
    if not z21.connected:
        raise HTTPException(status_code=503, detail="Z21 not connected")
    
    if not (1 <= address <= 9999):
        raise HTTPException(status_code=400, detail="Address must be 1-9999")
    
    info = await z21.get_loco_info(address)
    
    if not info:
        raise HTTPException(status_code=500, detail="Failed to get locomotive info")
    
    return info


@router.post("/emergency-stop")
async def emergency_stop(
    request: EmergencyStopRequest,
    z21: Z21Driver = Depends(get_z21)
):
    """Send emergency stop to all locomotives"""
    if not request.confirm:
        raise HTTPException(status_code=400, detail="Emergency stop not confirmed")
    
    if not z21.connected:
        raise HTTPException(status_code=503, detail="Z21 not connected")
    
    success = await z21.emergency_stop()
    
    if not success:
        raise HTTPException(status_code=500, detail="Failed to send emergency stop")
    
    return {
        "status": "emergency_stop_sent",
        "message": "All locomotives stopped",
    }