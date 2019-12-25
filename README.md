
# PACT
<img src="https://github.com/sas41/ProcessAffinityControlTool/blob/master/icon/PACT%20Logo.png?raw=true" width="256">

PACT is a tool designed to give the user a tighter control over their system by selecting which programs runs on which core.

# [Download <img src="https://github.com/sas41/ProcessAffinityControlTool/blob/master/icon/PACT%20Logo.png?raw=true" width="48">](https://github.com/sas41/ProcessAffinityControlTool/releases)

If you find this tool helpful and wish to donate, you can do so here: https://www.paypal.me/sas41

# Usage:

    add
        Adds an exception for a program, to run with a
        different core affinity and priority.
        
        This works best when used for games to run on their
        own cores/threads while the rest of the programs
        run on the defaults.
        
        Syntax:
            add [exe_name] [priority] [cores]
                Priority:
                    0: Idle (Not Recommended)
                    1: Below Normal
                    2: Normal
                    3: Above Normal
                    4: High
                    5: Real Time (Not Recommended)
                Cores:
                    Can be from 0 to howevermany you have.
        Example:
            add firefox 2 0 3
            
            This will ensure every instance of [firefox]
            runs with [2-Normal] priority on cores [0 & 3].
-----------------------------------------------------------
    remove or rem
        Removes a program from the exceptions list.
        
        Syntax:
            remove [exe_name]
            
        Example:
            remove firefox
-----------------------------------------------------------
    default_cores or dc
        Sets the default cores for all non-exception
        programs.
        
        Syntax:
            default_cores [cores]
                Cores:
                    Can be from 0 to howevermany you have.
            
        Example:
            default_cores 0 1 2 5
-----------------------------------------------------------
    default_priority or dp
        Sets the default priority for all non-exception
        programs.
        
        Syntax:
            default_priority [priority]
                Priority:
                    0: Idle (Not Recommended)
                    1: Below Normal
                    2: Normal
                    3: Above Normal
                    4: High
                    5: Real Time (Not Recommended)
            
        Example:
            default_priority 1
-----------------------------------------------------------
    scan_interval or si
       Set the scan interval in miliseconds.
       PACT will scan for new running programs
       every [x] ammount of miliseconds.
        
        Syntax:
            scan_interval [miliseconds]
            
        Example:
            scan_interval 3000
-----------------------------------------------------------
    aggressive_scan_interval or asi
       Set the aggresive scan interval.
       PACT will set processor affinity and priority
       for all running programs once fore every x
       amount of normal scans.
        
        Syntax:
            aggressive_scan_interval [normal_scan_count]
            
        Example:
            scan_interval 5

            Every 5th scan will be an aggressive scan.
-----------------------------------------------------------
    force_aggressive_scan or fas
       Makes every scan aggressive.
       Not recommended!
        
        Syntax:
            force_aggressive_scan [on, yes, true | off, no, false]
            
        Example:
            force_aggressive_scan false
-----------------------------------------------------------
    show_defaults or sd
        Show defaults.
        
        Syntax:
            show_defaults 
            
        Example:
            show_defaults
-----------------------------------------------------------
    show_exceptions or se
        Show a list of all exceptions.
        
        Syntax:
            show_exceptions 
            
        Example:
            show_exceptions
-----------------------------------------------------------
    toggle
        Toggles PACT, pauses or unpauses it's core
        functionality.
        
        Syntax:
            toggle 
            
        Example:
            toggle
-----------------------------------------------------------
    apply
        Applies changes immediately and resets timer.
        This is not required, as PACT will usually
        apply changes made on it's next scan.
        
        Syntax:
            apply 
            
        Example:
            apply
-----------------------------------------------------------
    save
        Saves the config to executable directory.
        On next run, this config will be applied.
        
        Syntax:
            save 
            
        Example:
            save
-----------------------------------------------------------
    exit or quit or stop
        Exits without saving.
        Sets all programs to system default priority
        and system default processor affinity.
        
        Syntax:
            exit 
            
        Example:
            exit
            
