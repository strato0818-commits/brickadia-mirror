class BuildingMirror {
  constructor(omegga, config, store) {
    this.omegga = omegga;
    this.config = config;
    this.store = store;

    this.playerData = {};
  }

  async init() {
    this.omegga
      .on('cmd:mirror', this.mirror);

    return {
      registeredCommands: ['mirror']
    };
  }

  unauthorized(senderName) {
    const player = this.omegga.getPlayer(senderName);
    if (
      this.config['only-authorized'] && !player.isHost() &&
      (
        (!this.config['authorized-users'] || !this.config['authorized-users'].some(p => player.id === p.id)) &&
        (!this.config['authorized-roles'] || !player.getRoles().some(role => this.config['authorized-roles'].includes(role)))
      )
    ) {
      this.omegga.whisper(senderName, '<color="ff0000">Unauthorized to use command.</>');
      return true;
    }
    return false;
  }

  rotate(rotation, turns) {
    return (rotation + turns) % 4;
  }

  shouldResize(rotation, rot) {
    return (rot + rotation) % 2 === 1;
  }

  convertDirection({ direction, rotation, size }, axis, brickType) {
    const rotationType = rotationTypes[brickType] || 1;
    const map = axisMap[direction];
    let dir = direction;
    let rot = rotation;
    let scale = size;
    if (axis[map.axis]) {
      dir = map.directionFlip;
      if (rotationType === 1) {
        rot = this.rotate(rot, rot % 2 ? map.flip ? 0 : 2 : map.flip ? 2 : 0);
      } else if (rotationType === 2) {
        rot = this.rotate(rot, rot % 2 ? map.flip ? 1 : 3 : map.flip ? 3 : 1);
      } else if (rotationType === 3) {
        rot = this.rotate(rot, rot % 2 ? map.flip ? 3 : 1 : map.flip ? 1 : 3);
      } else if (rotationType === 4) {
        rot = this.rotate(rot, rot % 2 ? map.flip ? 2 : 0 : map.flip ? 0 : 2);
      }
    }
    if (axis[map.turn1]) {
      if (rotationType === 1) {
        rot = this.rotate(rot, rot % 2 ? 2 : 0);
      } else if (rotationType === 2) {
        rot = this.rotate(rot, rot % 2 ? 3 : 1);
      } else if (rotationType === 3) {
        rot = this.rotate(rot, rot % 2 ? 1 : 3);
      } else if (rotationType === 4) {
        rot = this.rotate(rot, rot % 2 ? 0 : 2);
      }

    }
    if (axis[map.turn3]) {
      if (rotationType === 1) {
        rot = this.rotate(rot, rot % 2 ? 0 : 2);
      } else if (rotationType === 2) {
        rot = this.rotate(rot, rot % 2 ? 1 : 3);
      } else if (rotationType === 3) {
        rot = this.rotate(rot, rot % 2 ? 3 : 1);
      } else if (rotationType === 4) {
        rot = this.rotate(rot, rot % 2 ? 2 : 0);
      }
    }
    if (this.shouldResize(rotation, rot)) {
      scale = [size[1], size[0], size[2]];
    }
    return { direction: dir, rotation: rot, size: scale };
  }

  mirror = async (senderName, axisString) => {
    try {
      if (this.unauthorized(senderName)) return;
      if (axisString) {
        axisString = axisString.toLowerCase();
        const axis = [axisString.includes('x'), axisString.includes('y'), axisString.includes('z')];
        if ((axis[0] || axis[1] || axis[2])) {
          const player = this.omegga.getPlayer(senderName);
          const nameColor = player.getNameColor();
          this.omegga.broadcast(`<b><color="${nameColor}">${senderName}</></> mirroring selection (${axis[0] ? 'X' : ''}${axis[1] ? 'Y' : ''}${axis[2] ? 'Z' : ''})...`);
          let saveData = await player.getTemplateBoundsData();

          if (!saveData) {
            this.omegga.whisper(senderName, 'No bricks in selection.');
            return;
          }

          const { maxBound, center, minBound } = global.OMEGGA_UTIL.brick.getBounds(saveData);

          saveData.brick_assets = saveData.brick_assets.map((brickName) => mirrorMap[brickName] ? mirrorMap[brickName] : brickName);

          saveData.bricks = saveData.bricks.map((brick) => {
            const brickName = saveData.brick_assets[brick.asset_name_index]
            const { direction, rotation, size } = this.convertDirection(brick, axis, brickName);
            return {
              ...brick,
              position: brick.position.map((val, index) => (axis[index] ? maxBound[index] - val : val)),
              direction,
              rotation,
              size
            };
          });

          const offset = (i) => axis[i] ? maxBound[i] - (maxBound[i] - minBound[i]) : 0
          await player.loadSaveData(saveData, { offX: offset(0), offY: offset(1), offZ: offset(2) });
        } else {
          this.omegga.whisper(senderName, 'Enter a valid axis to mirror over: X, Y, or Z');
        }
      } else {
        this.omegga.whisper(senderName, 'Missing mirror axis: X, Y, or Z');
      }
    } catch (e) {
      console.log(`plugin error is caused by ${senderName}`, e);
    }
  }

  stop() {
    this.omegga
      .removeListener('cmd:mirror', this.mirror);
  }
}

const rotationTypes = {
  PB_DefaultBrick: 1,
  PB_DefaultRamp: 1,
  PB_DefaultRampCrest: 1,
  PB_DefaultRampCrestCorner: 2,
  PB_DefaultRampCrestEnd: 4,
  PB_DefaultRampInnerCorner: 3,
  PB_DefaultRampInnerCornerInverted: 3,
  PB_DefaultRampInverted: 1,
  PB_DefaultRampCorner: 3,
  PB_DefaultRampCornerInverted: 3,
  PB_DefaultSideWedge: 3,
  PB_DefaultTile: 1,
  PB_DefaultWedge: 1,
  PB_DefaultSideWedgeTile: 3,
  PB_DefaultMicroBrick: 1,
  PB_DefaultMicroWedgeCorner: 3,
  PB_DefaultMicroWedgeInnerCorner: 3,
  PB_DefaultMicroWedgeOuterCorner: 3,
  PB_DefaultMicroWedgeTriangleCorner: 3,
  PB_DefaultMicroWedge: 3,
  PB_DefaultMicroRamp: 1,
  PB_DefaultStudded: 1,
  B_1x1_Brick_Side: 4,
  B_1x1_Brick_Side_Lip: 1,
  B_1x1_Cone: 0,
  B_1x1_Round: 0,
  B_1x1F_Octo: 1,
  B_1x1F_Round: 0,
  B_1x2_Overhang: 1,
  B_1x2f_Plate_Center: 1,
  B_1x2f_Plate_Center_Inv: 1,
  B_1x4_Brick_Side: 4,
  B_1x_Octo: 1,
  B_1x_Octo_90Deg: 4,
  B_1x_Octo_90Deg_Inv: 4,
  B_1x_Octo_T: 4,
  B_1x_Octo_T_Inv: 4,
  B_2x1_Slipper: 1,
  B_2x2_Cone: 0,
  B_2x2_Corner: 3,
  B_2x2_Overhang: 1,
  B_2x2_Round: 0,
  B_2x2_Slipper: 1,
  B_2x2F_Octo: 1,
  B_2x2F_Octo_Converter: 1,
  B_2x2F_Octo_Converter_Inv: 1,
  B_2x2f_Plate_Center: 1,
  B_2x2f_Plate_Center_Inv: 1,
  B_2x2F_Round: 0,
  B_2x4_Door_Frame: 1,
  B_2x_Cube_Side: 0,
  B_2x_Octo: 1,
  B_2x_Octo_90Deg: 4,
  B_2x_Octo_90Deg_Inv: 4,
  B_2x_Octo_Cone: 0,
  B_2x_Octo_T: 4,
  B_2x_Octo_T_Inv: 4,
  B_4x4_Round: 0,
  B_8x8_Lattice_Plate: 0,
  B_Bishop: 0,
  B_Bone: 2,
  B_BoneStraight: 1,
  B_Branch: 1,
  B_Bush: 0,
  B_Cauldron: 0,
  B_Chalice: 0,
  B_CheckPoint: 1,
  B_Coffin: 1,
  B_Leaf_Bush: 1,
  B_Coffin_Lid: 1,
  B_Fern: 2,
  B_Flame: 0,
  B_Flower: 0,
  B_Gravestone: 0,
  B_GoalPoint: 1,
  B_Handle: 1,
  B_Hedge_1x1: 1,
  B_Hedge_1x1_Corner: 3,
  B_Hedge_1x2: 1,
  B_Hedge_1x4: 1,
  B_Inverted_Cone: 0,
  B_Jar: 0,
  B_King: 4,
  B_Knight: 4,
  B_Ladder: 1,
  B_Pawn: 0,
  B_Picket_Fence: 1,
  B_Pine_Tree: 0,
  B_Pumpkin: 4,
  B_Pumpkin_Carved: 4,
  B_Queen: 4,
  B_Rook: 4,
  B_Sausage: 1,
  B_Small_Flower: 0,
  B_SpawnPoint: 1,
  B_Swirl_Plate: 1,
  B_Turkey_Body: 1,
  B_Turkey_Leg: 1,
  PB_DefaultArch: 1,
  PB_DefaultArchInverted: 1,
  PB_DefaultPole: 1,
  PB_DefaultMicroWedgeHalfInnerCorner: 3,
  PB_DefaultMicroWedgeHalfInnerCornerInverted: 3,
  PB_DefaultMicroWedgeHalfOuterCorner: 3,
  // EA types
  B_Joint_Wheel: 1,
  B_Vehicle_Engine: 1,
  PBG_ZoneProjector: 1,
  BP_ZoneProjector: 1,
  B_Joint_Coupler: 1,
  B_Button_Pressed: 1,
  B_Button_Square: 1,
  B_Button_Square_Pressed: 1,
  B_Switch_Test: 4,
  B_Switch_Flipped: 4,
  B_1x1_SoundEmitter: 4,
  B_2x2_Thruster: 1,
  B_1x1_Gate_Clock: 4,
  B_2x2F_Target: 1,
  B_1x1_Gate_SeatControlSplitter: 4,
  B_Bot_Spawn_Point: 1,
  B_1x1_Gate_WeightBrick: 4,
  B_Capture_Point: 1,
  B_Joint_Motor_Micro: 1,
  B_Joint_Servo: 1,
  B_Joint_Servo_Micro: 1,
  B_1x1_EventGate_Generic_2: 4,
  B_1x1_Gate_GreaterThanEqual: 4,
  PB_RoundedCap: 1,
  PBG_RoundedCap: 1,
  B_1x1f_Inverse_Tile_Corner: 1,
  B_1x1_Gate_OR: 4,
  B_Joint_Bearing: 1,
  B_Joint_Motor: 1,
  PBG_MotorSliderJoint: 1,
  PB_MotorSliderJoint: 1,
  B_1x1_Reroute_Node: 4,
  PBG_SliderJoint: 1,
  PB_SliderJoint: 1,
  B_Joint_Bearing_Micro: 1,
  B_Joint_Spring: 1,
  B_Joint_Wheel_Micro: 1,
  PBG_ServoSliderJoint: 1,
  PB_ServoSliderJoint: 1,
  B_1x1_EntityGate_SetInventorySlot: 4,
  B_1x1_EntityGate_PlayAudioAt: 4,
  PB_Spike: 1,
  PBG_Spike: 1,
  B_1x1F_Speaker: 1,
  B_2x2F_Speaker: 1,
  B_Joint_Socket_Micro: 1,
  B_Joint_Socket: 1,
  B_1x2_MetalIngot: 1,
  B_1x1_EntityGate_AddVelocity: 4,
  B_1x1_EventGate_Zone_BrickUpdated: 4,
  B_DestinationPoint: 1,
  B_1x1_Gate_NOR_Bitwise: 4,
  B_1x1_Gate_ShiftLeft_Bitwise: 4,
  B_1x1_Gate_LessThanEqual: 4,
  B_1x1_Gate_Ceiling: 4,
  B_1x1_Gate_GreaterThan: 4,
  B_1x1_Gate_NAND: 4,
  B_1x1_Gate_Blend: 4,
  B_1x1_Gate_Add: 4,
  B_1x1_Gate_Multiply: 4,
  B_1x1_Gate_Timer_Tick: 4,
  B_1x1_Gate_Divide: 4,
  BP_SquarePlate: 1,
  PBG_RoundPlate: 1,
  PBG_SquarePlate: 1,
  BP_RoundPlate: 1,
  B_1x1_Gate_MemoryCell: 4,
  BCP_WedgeTest: 1,
  PBG_WedgeTest: 1,
  PB_WedgeTest: 1,
  B_1x1_Gate_BrickEditor: 4,
  B_1x1_Gate_Receiver: 4,
  B_1x1_Gate_Transmitter: 4,
  B_1x1_ParticleEmitter: 4,
  B_1x1_Gate_WheelEngineSlim: 1,
  B_1x1_Coin: 1,
  B_1x1_Coin_Diagonal: 1,
  B_1x1f_Tile_Corner: 1,
  B_Frog_Small: 1,
  B_1x1_Microchip: 1,
  B_1x1_Gate_Blank_1I: 4,
  B_1x1_Gate_Blank_1I_1O: 4,
  B_1x1_Gate_Blank_1I_2O: 4,
  B_1x1_Gate_Blank_1O: 4,
  B_1x1_Gate_Blank_2I_1O: 4,
  B_1x1_Gate_Blank_3I_1O: 4,
  B_1x1_EventGate_Generic: 4,
  B_1x1_Gate_ExecUnion: 4,
  B_1x1_EntityGate_SetGameplayPermission: 4,
  B_1x1_EntityGate_ShowText: 4,
  B_1x1_CharacterGate_SetGravityDirection: 4,
  B_1x1_EntityGate_AddLocationAndRotation: 4,
  B_1x1_EntityGate_ClearBricks: 4,
  B_1x1_EntityGate_SetLocation: 4,
  B_1x1_EntityGate_SetLocationAndRotation: 4,
  B_1x1_EntityGate_SetRotation: 4,
  B_1x1_EntityGate_SetVelocity: 4,
  B_1x1_Gate_RelativeTeleport: 4,
  B_1x1_Gate_Teleport: 4,
  B_1x1_EventGate_Zone_BrickRemoved: 4,
  B_1x1_EventGate_Zone_CharacterEntered: 4,
  B_1x1_EventGate_Zone_PlayerLeft: 4,
  B_1x1_Gate_AND_Bitwise: 4,
  B_1x1_Gate_NAND_Bitwise: 4,
  B_1x1_Gate_NOT_Bitwise: 4,
  B_1x1_Gate_OR_Bitwise: 4,
  B_1x1_Gate_ShiftRight_Bitwise: 4,
  B_1x1_Gate_XOR_Bitwise: 4,
  B_1x1_Gate_Equal: 4,
  B_1x1_Gate_LessThan: 4,
  B_1x1_Gate_NotEqual: 4,
  B_1x1_EntityGate_ReadBrickGrid: 4,
  B_1x1_Gate_Floor: 4,
  B_1x1_Gate_AND: 4,
  B_1x1_Gate_EdgeDetector: 4,
  B_1x1_Gate_NOR: 4,
  B_1x1_Gate_XOR: 4,
  B_1x1_NOT_Gate: 4,
  B_1x1_Gate_Mod: 4,
  B_1x1_Gate_ModFloored: 4,
  B_1x1_Gate_Subtract: 4,
  B_1x1_Gate_Timer: 4,
  B_1x1_Gate_Constant: 4,
  PBG_SpikePlate: 1,
  BP_SpikePlate: 1,
  PBG_PicketFence: 1,
  PB_PicketFence: 1,
  PBG_LatticeThin: 1,
  BP_LatticeThin: 1,
  PBG_Baguette: 1,
  PB_Baguette: 1,
  B_Fork: 4,
  B_Spoon: 4,
  B_Seat: 4,
  PB_DefaultSmoothTile: 1,
};

const mirrorMap = {
  PB_DefaultMicroWedgeHalfInnerCorner: 'PB_DefaultMicroWedgeHalfInnerCornerInverted',
  PB_DefaultMicroWedgeHalfInnerCornerInverted: 'PB_DefaultMicroWedgeHalfInnerCorner',
}

const axisMap = {
  0: {
    axis: 0,
    flip: false,
    turn1: 1,
    turn3: 2,
    directionFlip: 1
  },
  1: {
    axis: 0,
    flip: false,
    turn1: 1,
    turn3: 2,
    directionFlip: 0
  },
  2: {
    axis: 1,
    flip: false,
    turn1: 0,
    turn3: 2,
    directionFlip: 3
  },
  3: {
    axis: 1,
    flip: false,
    turn1: 0,
    turn3: 2,
    directionFlip: 2
  },
  4: {
    axis: 2,
    flip: true,
    turn1: 1,
    turn3: 0,
    directionFlip: 5
  },
  5: {
    axis: 2,
    flip: true,
    turn1: 1,
    turn3: 0,
    directionFlip: 4
  },
};


module.exports = BuildingMirror;